use std::{collections::HashMap, io::BufRead, iter, sync::Arc};

use rand::Rng;
use reqwest::{header, Response};
use simd_json::{
	base::{ValueAsContainer, ValueAsScalar},
	OwnedValue
};
use tokio::{
	sync::{broadcast, Mutex},
	task::JoinHandle
};
use url::Url;

use super::{types::GetLiveChatResponse, util::SimdJsonResponseBody, YouTubeError};

const GCM_SIGNALER_SRQE: &str = "https://signaler-pa.youtube.com/punctual/v1/chooseServer";
const GCM_SIGNALER_PSUB: &str = "https://signaler-pa.youtube.com/punctual/multi-watch/channel";

const LIVE_CHAT_BASE_TANGO_KEY: &str = "AIzaSyDZNkyC-AtROwMBpLfevIvqYk-Gfi8ZOeo";

#[derive(Debug, Default)]
struct SignalerChannelInner {
	topic: String,
	gsessionid: Option<String>,
	sid: Option<String>,
	rid: usize,
	aid: usize,
	session_n: usize
}

impl SignalerChannelInner {
	pub fn with_topic(topic: impl ToString) -> Self {
		Self {
			topic: topic.to_string(),
			..Default::default()
		}
	}

	pub fn reset(&mut self) {
		self.gsessionid = None;
		self.sid = None;
		self.rid = 0;
		self.aid = 0;
		self.session_n = 0;
	}

	fn gen_zx() -> String {
		const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
		let mut rng = rand::thread_rng();
		iter::repeat_with(|| CHARSET[rng.gen_range(0..CHARSET.len())] as char).take(11).collect()
	}

	pub async fn choose_server(&mut self) -> Result<(), YouTubeError> {
		let server_response: OwnedValue = super::get_http_client()
			.post(Url::parse_with_params(GCM_SIGNALER_SRQE, [("key", LIVE_CHAT_BASE_TANGO_KEY)])?)
			.header(header::CONTENT_TYPE, "application/json+protobuf")
			.body(format!(r#"[[null,null,null,[7,5],null,[["youtube_live_chat_web"],[1],[[["{}"]]]]]]"#, self.topic))
			.send()
			.await?
			.simd_json()
			.await?;
		let gsess = server_response.as_array().unwrap()[0].as_str().unwrap();
		self.gsessionid = Some(gsess.to_owned());
		Ok(())
	}

	pub async fn init_session(&mut self) -> Result<(), YouTubeError> {
		let mut ofs_parameters = HashMap::new();
		ofs_parameters.insert("count", "1".to_string());
		ofs_parameters.insert("ofs", "0".to_string());
		ofs_parameters.insert(
			"req0___data__",
			format!(r#"[[["1",[null,null,null,[7,5],null,[["youtube_live_chat_web"],[1],[[["{}"]]]],null,null,1],null,3]]]"#, self.topic)
		);
		self.session_n = 1;
		let ofs = super::get_http_client()
			.post(Url::parse_with_params(
				GCM_SIGNALER_PSUB,
				[
					("VER", "8"),
					("gsessionid", self.gsessionid.as_ref().unwrap()),
					("key", LIVE_CHAT_BASE_TANGO_KEY),
					("RID", &self.rid.to_string()),
					("AID", &self.aid.to_string()),
					("CVER", "22"),
					("zx", Self::gen_zx().as_ref()),
					("t", "1")
				]
			)?)
			.header("X-WebChannel-Content-Type", "application/json+protobuf")
			.form(&ofs_parameters)
			.send()
			.await?;

		let mut ofs_res_line = ofs.bytes().await?.lines().nth(1).unwrap().unwrap();
		let value: OwnedValue = unsafe { simd_json::from_str(&mut ofs_res_line) }?;
		let value = value.as_array().unwrap()[0].as_array().unwrap();
		assert_eq!(value[0].as_usize().unwrap(), 0);
		let sid = value[1].as_array().unwrap()[1].as_str().unwrap();
		self.sid = Some(sid.to_owned());
		Ok(())
	}

	pub async fn get_session_stream(&self) -> Result<Response, YouTubeError> {
		Ok(super::get_http_client()
			.get(Url::parse_with_params(
				GCM_SIGNALER_PSUB,
				[
					("VER", "8"),
					("gsessionid", self.gsessionid.as_ref().unwrap()),
					("key", LIVE_CHAT_BASE_TANGO_KEY),
					("RID", "rpc"),
					("SID", self.sid.as_ref().unwrap()),
					("AID", &self.aid.to_string()),
					("CI", "0"),
					("TYPE", "xmlhttp"),
					("zx", &Self::gen_zx()),
					("t", "1")
				]
			)?)
			.header(header::CONNECTION, "keep-alive")
			.send()
			.await?)
	}
}

#[derive(Debug)]
pub struct SignalerChannel {
	inner: Arc<Mutex<SignalerChannelInner>>
}

impl SignalerChannel {
	pub async fn new(topic_id: impl ToString) -> Result<Self, YouTubeError> {
		Ok(SignalerChannel {
			inner: Arc::new(Mutex::new(SignalerChannelInner::with_topic(topic_id)))
		})
	}

	pub async fn new_from_cont(cont: &GetLiveChatResponse) -> Result<Self, YouTubeError> {
		Ok(SignalerChannel {
			inner: Arc::new(Mutex::new(SignalerChannelInner::with_topic(
				&cont.continuation_contents.as_ref().unwrap().live_chat_continuation.continuations[0]
					.invalidation_continuation_data
					.as_ref()
					.unwrap()
					.invalidation_id
					.topic
			)))
		})
	}

	pub async fn refresh_topic(&self, topic: impl ToString) {
		self.inner.lock().await.topic = topic.to_string();
	}

	pub async fn spawn_event_subscriber(&self) -> Result<(broadcast::Receiver<()>, JoinHandle<()>), YouTubeError> {
		let inner = Arc::clone(&self.inner);
		{
			let mut lock = inner.lock().await;
			lock.choose_server().await?;
			lock.init_session().await?;
		}
		let (sender, receiver) = broadcast::channel(128);
		let handle = tokio::spawn(async move {
			'i: loop {
				let mut req = {
					let mut lock = inner.lock().await;
					let _ = sender.send(());
					lock.reset();
					lock.choose_server().await.unwrap();
					lock.init_session().await.unwrap();
					lock.get_session_stream().await.unwrap()
				};
				loop {
					match req.chunk().await {
						Ok(None) => break,
						Ok(Some(s)) => {
							let mut ofs_res_line = s.lines().nth(1).unwrap().unwrap();
							if let Ok(s) = unsafe { simd_json::from_str::<OwnedValue>(ofs_res_line.as_mut()) } {
								let a = s.as_array().unwrap();
								{
									inner.lock().await.aid = a[a.len() - 1].as_array().unwrap()[0].as_usize().unwrap();
								}
							}

							if sender.send(()).is_err() {
								break 'i;
							}
						}
						Err(e) => {
							eprintln!("{e:?}");
							break;
						}
					}
				}
			}
		});
		Ok((receiver, handle))
	}
}
