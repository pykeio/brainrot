use std::{
	collections::{HashMap, VecDeque},
	io::BufRead,
	sync::OnceLock,
	time::{Instant, SystemTime, UNIX_EPOCH}
};

use regex::Regex;
use reqwest::{
	get,
	header::{self, HeaderMap, HeaderValue, CONTENT_TYPE},
	StatusCode
};
use simd_json::{
	base::{ValueAsContainer, ValueAsScalar},
	OwnedValue
};
use thiserror::Error;
use tokio::sync::Mutex;

mod types;
mod util;
use self::{
	types::{Action, GetLiveChatBody, GetLiveChatResponse, MessageRun},
	util::{SimdJsonRequestBody, SimdJsonResponseBody}
};

const GCM_SIGNALER_SRQE: &str = "https://signaler-pa.youtube.com/punctual/v1/chooseServer";
const GCM_SIGNALER_PSUB: &str = "https://signaler-pa.youtube.com/punctual/multi-watch/channel";

const LIVE_CHAT_BASE_TANGO_KEY: &str = "AIzaSyDZNkyC-AtROwMBpLfevIvqYk-Gfi8ZOeo";

static USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:123.0) Gecko/20100101 Firefox/123.0";
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[derive(Debug, Error)]
pub enum YouTubeError {
	#[error("impossible regex error")]
	Regex(#[from] regex::Error),
	#[error("error when deserializing: {0}")]
	Deserialization(#[from] simd_json::Error),
	#[error("missing continuation contents")]
	MissingContinuationContents,
	#[error("reached end of continuation")]
	EndOfContinuation,
	#[error("request timed out")]
	TimedOut,
	#[error("request returned bad HTTP status: {0}")]
	BadStatus(StatusCode),
	#[error("request error: {0}")]
	GeneralRequest(reqwest::Error),
	#[error("{0} is not a live stream")]
	NotStream(String),
	#[error("Failed to match InnerTube API key")]
	NoInnerTubeKey,
	#[error("Chat continuation token could not be found.")]
	NoChatContinuation
}

impl From<reqwest::Error> for YouTubeError {
	fn from(value: reqwest::Error) -> Self {
		if value.is_timeout() {
			YouTubeError::TimedOut
		} else if value.is_status() {
			YouTubeError::BadStatus(value.status().unwrap())
		} else {
			YouTubeError::GeneralRequest(value)
		}
	}
}

pub(crate) fn get_http_client() -> &'static reqwest::Client {
	HTTP_CLIENT.get_or_init(|| {
		let mut headers = HeaderMap::new();
		// Set our Accept-Language to en-US so we can properly match substrings
		headers.append(header::ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
		headers.append(header::USER_AGENT, HeaderValue::from_static(USER_AGENT));
		reqwest::Client::builder().default_headers(headers).build().unwrap()
	})
}

#[derive(Clone, Debug)]
pub struct RequestOptions {
	pub api_key: String,
	pub client_version: String,
	pub live_status: bool
}

pub async fn get_options_from_live_page(live_id: impl AsRef<str>) -> Result<(RequestOptions, String), YouTubeError> {
	let live_id = live_id.as_ref();
	let page_contents = get_http_client()
		.get(format!("https://www.youtube.com/watch?v={live_id}"))
		.send()
		.await?
		.text()
		.await?;

	let live_status: bool;
	let live_now_regex = Regex::new(r#"['"]isLiveNow['"]:\s*(true)"#)?;
	let not_replay_regex = Regex::new(r#"['"]isReplay['"]:\s*(true)"#)?;
	if live_now_regex.find(&page_contents).is_some() {
		live_status = true;
	} else if not_replay_regex.find(&page_contents).is_some() {
		live_status = false;
	} else {
		return Err(YouTubeError::NotStream(live_id.to_string()));
	}

	let api_key_regex = Regex::new(r#"['"]INNERTUBE_API_KEY['"]:\s*['"](.+?)['"]"#).unwrap();
	let api_key = match api_key_regex.captures(&page_contents).and_then(|captures| captures.get(1)) {
		Some(matched) => matched.as_str().to_string(),
		None => return Err(YouTubeError::NoInnerTubeKey)
	};

	let client_version_regex = Regex::new(r#"['"]clientVersion['"]:\s*['"]([\d.]+?)['"]"#).unwrap();
	let client_version = match client_version_regex.captures(&page_contents).and_then(|captures| captures.get(1)) {
		Some(matched) => matched.as_str().to_string(),
		None => "2.20230801.08.00".to_string()
	};

	let continuation_regex = if live_status {
		Regex::new(
			r#"Live chat['"],\s*['"]selected['"]:\s*(?:true|false),\s*['"]continuation['"]:\s*\{\s*['"]reloadContinuationData['"]:\s*\{['"]continuation['"]:\s*['"](.+?)['"]"#
		)?
	} else {
		Regex::new(
			r#"Top chat replay['"],\s*['"]selected['"]:\s*true,\s*['"]continuation['"]:\s*\{\s*['"]reloadContinuationData['"]:\s*\{['"]continuation['"]:\s*['"](.+?)['"]"#
		)?
	};
	let continuation = match continuation_regex.captures(&page_contents).and_then(|captures| captures.get(1)) {
		Some(matched) => matched.as_str().to_string(),
		None => return Err(YouTubeError::NoChatContinuation)
	};

	Ok((RequestOptions { api_key, client_version, live_status }, continuation))
}
pub struct Author {
	pub display_name: String,
	pub id: String,
	pub avatar: String
}

pub struct ChatMessage {
	pub runs: Vec<MessageRun>,
	pub is_super: bool,
	pub author: Author,
	pub timestamp: i64,
	pub time_delta: i64
}

pub struct YouTubeChatPageProcessor<'r> {
	actions: Mutex<VecDeque<Action>>,
	request_options: &'r RequestOptions,
	continuation_token: Option<String>
}

#[derive(Debug, Error)]
#[error("no continuation available")]
pub struct NoContinuationError;

unsafe impl<'r> Send for YouTubeChatPageProcessor<'r> {}

impl<'r> YouTubeChatPageProcessor<'r> {
	pub fn new(response: GetLiveChatResponse, request_options: &'r RequestOptions, continuation_token: Option<String>) -> Result<Self, YouTubeError> {
		Ok(Self {
			actions: Mutex::new(VecDeque::from(
				response
					.continuation_contents
					.ok_or(YouTubeError::MissingContinuationContents)?
					.live_chat_continuation
					.actions
					.ok_or(YouTubeError::EndOfContinuation)?
			)),
			request_options,
			continuation_token
		})
	}
}

impl<'r> Iterator for &YouTubeChatPageProcessor<'r> {
	type Item = ChatMessage;

	fn next(&mut self) -> Option<Self::Item> {
		let mut next_action = None;
		while next_action.is_none() {
			match self.actions.try_lock().unwrap().pop_front() {
				Some(action) => {
					if let Some(replay) = action.replay_chat_item_action {
						for action in replay.actions {
							if next_action.is_some() {
								break;
							}

							if let Some(add_chat_item_action) = action.add_chat_item_action {
								if let Some(text_message_renderer) = &add_chat_item_action.item.live_chat_text_message_renderer {
									if text_message_renderer.message.is_some() {
										next_action.replace((add_chat_item_action, replay.video_offset_time_msec));
									}
								} else if let Some(superchat_renderer) = &add_chat_item_action.item.live_chat_paid_message_renderer {
									if superchat_renderer.live_chat_text_message_renderer.message.is_some() {
										next_action.replace((add_chat_item_action, replay.video_offset_time_msec));
									}
								}
							}
						}
					}
				}
				None => return None
			}
		}

		let (next_action, time_delta) = next_action.unwrap();
		let is_super = next_action.item.live_chat_paid_message_renderer.is_some();
		let renderer = if let Some(renderer) = next_action.item.live_chat_text_message_renderer {
			renderer
		} else if let Some(renderer) = next_action.item.live_chat_paid_message_renderer {
			renderer.live_chat_text_message_renderer
		} else {
			panic!()
		};

		Some(ChatMessage {
			runs: renderer.message.unwrap().runs,
			is_super,
			author: Author {
				display_name: renderer
					.message_renderer_base
					.author_name
					.map(|x| x.simple_text)
					.unwrap_or_else(|| renderer.message_renderer_base.author_external_channel_id.to_owned()),
				id: renderer.message_renderer_base.author_external_channel_id.to_owned(),
				avatar: renderer.message_renderer_base.author_photo.thumbnails[renderer.message_renderer_base.author_photo.thumbnails.len() - 1]
					.url
					.to_owned()
			},
			timestamp: renderer.message_renderer_base.timestamp_usec.timestamp_millis(),
			time_delta
		})
	}
}

pub async fn fetch_yt_chat_page(options: &RequestOptions, continuation: impl AsRef<str>) -> Result<GetLiveChatResponse, YouTubeError> {
	let url =
		format!("https://www.youtube.com/youtubei/v1/live_chat/get_live_chat{}?key={}", if !options.live_status { "_replay" } else { "" }, &options.api_key);
	let body = GetLiveChatBody::new(continuation.as_ref(), &options.client_version, "WEB");
	let response = get_http_client().post(url).simd_json(&body)?.send().await?;
	let response: GetLiveChatResponse = response.simd_json().await?;
	Ok(response)
}

pub async fn subscribe_to_events(options: &RequestOptions, initial_continuation: &GetLiveChatResponse) -> Result<(), YouTubeError> {
	let topic_id = &initial_continuation
		.continuation_contents
		.as_ref()
		.unwrap()
		.live_chat_continuation
		.continuations[0]
		.invalidation_continuation_data
		.as_ref()
		.unwrap()
		.invalidation_id
		.topic;

	let server_response: OwnedValue = get_http_client()
		.post(format!("{GCM_SIGNALER_SRQE}?key={}", LIVE_CHAT_BASE_TANGO_KEY))
		.header(header::CONTENT_TYPE, "application/json+protobuf")
		.header(header::REFERER, "https://www.youtube.com/")
		.header("Sec-Fetch-Site", "same-site")
		.header(header::ORIGIN, "https://www.youtube.com/")
		.header(header::ACCEPT_ENCODING, "gzip, deflate, br")
		.simd_json(&simd_json::json!([[null, null, null, [7, 5], null, [["youtube_live_chat_web"], [1], [[[&topic_id]]]]]]))?
		.send()
		.await?
		.simd_json()
		.await?;
	let gsess = server_response.as_array().unwrap()[0].as_str().unwrap();

	let mut ofs_parameters = HashMap::new();
	ofs_parameters.insert("count", "1".to_string());
	ofs_parameters.insert("ofs", "0".to_string());
	ofs_parameters.insert(
		"req0___data__",
		format!(
			r#"[[["1",[null,null,null,[7,5],null,[["youtube_live_chat_web"],[1],[[["{}"]]]],null,null,1],null,3]]]"#,
			&topic_id
		)
	);
	let ofs = get_http_client()
		.post(format!("{GCM_SIGNALER_PSUB}?VER=8&gsessionid={gsess}&key={LIVE_CHAT_BASE_TANGO_KEY}&RID=60464&CVER=22&zx=uo5vp9j380ef&t=1"))
		.header(header::REFERER, "https://www.youtube.com/")
		.header("Sec-Fetch-Site", "same-site")
		.header(header::ORIGIN, "https://www.youtube.com/")
		.header(header::ACCEPT_ENCODING, "gzip, deflate, br")
		.header("X-WebChannel-Content-Type", "application/json+protobuf")
		.form(&ofs_parameters)
		.send()
		.await?;

	let mut ofs_res_line = ofs.bytes().await?.lines().nth(1).unwrap().unwrap();
	let value: OwnedValue = unsafe { simd_json::from_str(&mut ofs_res_line) }?;
	let value = value.as_array().unwrap()[0].as_array().unwrap();
	assert_eq!(value[0].as_usize().unwrap(), 0);
	let sid = value[1].as_array().unwrap()[1].as_str().unwrap();

	let mut stream = get_http_client()
		.get(format!(
			"{GCM_SIGNALER_PSUB}?VER=8&gsessionid={gsess}&key={LIVE_CHAT_BASE_TANGO_KEY}&RID=rpc&SID={sid}&AID=0&CI=0&TYPE=xmlhttp&zx=uo5vp9j380ed&t=1"
		))
		.header(header::REFERER, "https://www.youtube.com/")
		.header("Sec-Fetch-Site", "same-site")
		.header(header::ORIGIN, "https://www.youtube.com/")
		.header(header::ACCEPT_ENCODING, "gzip, deflate, br")
		.header(header::ACCEPT, "*/*")
		.header(header::CONNECTION, "keep-alive")
		.send()
		.await?;

	while let Some(c) = stream.chunk().await? {
		println!("{}", String::from_utf8_lossy(&c));
	}

	Ok(())
}
