// Copyright 2025 pyke.io
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{error::Error as StdError, fmt, io::BufRead, iter};

use async_stream_lite::try_async_stream;
use bytes::Bytes;
use futures_util::Stream;
use http::{HeaderName, HeaderValue, Method, Uri, header, uri::PathAndQuery};
use simd_json::{
	OwnedValue,
	base::{ValueAsArray, ValueAsScalar}
};

use super::client::{Client, ClientError, RequestExecutor, Response};

#[derive(Debug, Default)]
pub struct SignalerChannel {
	topic: String,
	tango_key: String,
	gsessionid: Option<String>,
	sid: Option<String>,
	rid: usize,
	aid: usize
}

impl SignalerChannel {
	pub fn with_topic(topic: impl ToString, tango_key: impl ToString) -> Self {
		Self {
			topic: topic.to_string(),
			tango_key: tango_key.to_string(),
			..Default::default()
		}
	}

	fn reset(&mut self) {
		self.gsessionid = None;
		self.sid = None;
		self.rid = 0;
		self.aid = 0;
	}

	fn gen_zx() -> String {
		const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
		iter::repeat_with(|| CHARSET[fastrand::usize(0..CHARSET.len())] as char)
			.take(11)
			.collect()
	}

	async fn choose_server<E: RequestExecutor>(&mut self, client: &Client<E>) -> Result<(), SignalerError<E>> {
		let request = client
			.base_request(
				Uri::builder()
					.scheme("https")
					.authority("signaler-pa.youtube.com")
					.path_and_query(
						format!("/punctual/v1/chooseServer?key={}", self.tango_key)
							.parse::<PathAndQuery>()
							.expect("invalid path")
					)
					.build()
					.expect("invalid URI")
			)
			.method(Method::POST)
			.header(header::CONTENT_TYPE, HeaderValue::from_static("application/json+protobuf"))
			.body(format!(r#"[[null,null,null,[8,5],null,[["youtube_live_chat_web"],[1],[[["{}"]]]]]]"#, self.topic).into())
			.expect("invalid request");
		let mut server_response = client.execute(request).await?.recv_all().await.map_err(SignalerError::Receive)?;
		let server_response: simd_json::BorrowedValue<'_> = simd_json::from_slice(&mut server_response)?;

		if let Some(res) = server_response.as_array()
			&& let Some(gsess) = res.first().and_then(|x| x.as_str())
		{
			self.gsessionid = Some(gsess.to_owned());
		} else {
			return Err(SignalerError::Parse {
				source: SignalerParseSource::ChooseServer
			});
		}

		Ok(())
	}

	async fn init_session<E: RequestExecutor>(&mut self, client: &Client<E>) -> Result<(), SignalerError<E>> {
		let ofs_parameters = format!(
			// [[["1",[null,null,null,[8,5],null,[["youtube_live_chat_web"],[1],[[["{}"]]]],null,null,1],null,3]]]
			"count=1&ofs=0&req0___data__=%5B%5B%5B%221%22%2C%5Bnull%2Cnull%2Cnull%2C%5B8%2C5%5D%2Cnull%2C%5B%5B%22youtube_live_chat_web%22%5D%2C%5B1%5D%2C%5B%5B%5B%22{}%22%5D%5D%5D%5D%2Cnull%2Cnull%2C1%5D%2Cnull%2C3%5D%5D%5D",
			self.topic
		);

		let request = client
			.base_request(
				Uri::builder()
					.scheme("https")
					.authority("signaler-pa.youtube.com")
					.path_and_query(
						format!(
							"/punctual/multi-watch/channel?VER=8&gsessionid={gsi}&key={tango_key}&RID={rid}&CVER=22&zx={zx}&t=1",
							gsi = self.gsessionid.as_ref().expect("should have chosen server by now"),
							tango_key = self.tango_key,
							rid = "0", // self.rid
							zx = Self::gen_zx()
						)
						.parse::<PathAndQuery>()
						.expect("invalid path")
					)
					.build()
					.expect("invalid URI")
			)
			.method(Method::POST)
			.header(HeaderName::from_static("x-webchannel-content-type"), HeaderValue::from_static("application/json+protobuf"))
			.header(header::CONTENT_TYPE, HeaderValue::from_static("application/x-www-form-urlencoded"))
			.body(ofs_parameters.into())
			.expect("invalid request");
		let ofs = client.execute(request).await?.recv_all().await.map_err(SignalerError::Receive)?;

		let parse_err = Err(SignalerError::Parse {
			source: SignalerParseSource::SessionInit
		});
		let Some(Ok(mut res_line)) = ofs.lines().nth(1) else {
			return Err(SignalerError::Parse {
				source: SignalerParseSource::SessionInit
			});
		};
		let value: OwnedValue = unsafe { simd_json::from_str(&mut res_line) }?;

		let Some(data) = value.as_array().and_then(|x| x.first().and_then(|x| x.as_array())) else {
			return parse_err;
		};

		// first value might be 1 if the request has an error, not entirely sure
		if data.first().and_then(|x| x.as_usize()) != Some(0) {
			return parse_err;
		}

		let Some(sid) = data.get(1).and_then(|x| x.as_array()).and_then(|x| x.get(1).and_then(|x| x.as_str())) else {
			return parse_err;
		};
		self.sid = Some(sid.to_owned());

		Ok(())
	}

	pub async fn stream<E: RequestExecutor>(&mut self, client: &Client<E>) -> Result<impl Stream<Item = Result<(), SignalerError<E>>> + '_, SignalerError<E>> {
		// TODO: see if we can not need to reset state every time
		self.reset();
		self.choose_server(client).await?;
		self.init_session(client).await?;

		let request = client
			.base_request(
				Uri::builder()
					.scheme("https")
					.authority("signaler-pa.youtube.com")
					.path_and_query(
						format!(
							"/punctual/multi-watch/channel?VER=8&gsessionid={gsi}&key={tango_key}&RID=rpc&SID={sid}&AID={aid}&CI=0&TYPE=xmlhttp&zx={zx}&t=1",
							gsi = self.gsessionid.as_ref().expect("should have chosen server by now"),
							tango_key = self.tango_key,
							sid = self.sid.as_ref().expect("should have SID by now"),
							aid = self.aid,
							zx = Self::gen_zx()
						)
						.parse::<PathAndQuery>()
						.expect("invalid path")
					)
					.build()
					.expect("invalid URI")
			)
			.method(Method::GET)
			.header(header::CONNECTION, HeaderValue::from_static("keep-alive"))
			.body(Bytes::new())
			.expect("invalid request");
		let mut res = client.execute(request).await?;
		Ok(try_async_stream(|yielder| async move {
			loop {
				match res.recv_chunk().await {
					Ok(Some(chunk)) => {
						let mut lines = chunk.lines();
						let Some(Ok(event_id)) = lines.next() else {
							break;
						};

						if event_id != "252" && event_id != "253" && event_id != "254" {
							// 50, 51, and 53 are probably some internal stuff we don't care about. 25x seem to be correlated with new chat
							// messages (though sometimes there aren't new chat messages at all and I'm not sure why).
							// The channel starts off sending 252 but after a few seconds sends 253 instead, and in higher volume streams gets up to
							// 254. Not sure the difference between the events, but they're all structured & function the same.
							continue;
						}

						let Some(Ok(mut ofs_res_line)) = lines.next() else {
							break;
						};

						if let Ok(s) = unsafe { simd_json::from_str::<simd_json::OwnedValue>(ofs_res_line.as_mut()) }
							&& let Some(a) = s.as_array()
							&& let Some(aid) = a.last().and_then(|x| x.as_array()).and_then(|x| x.first()).and_then(|x| x.as_usize())
						{
							self.aid = aid;
						}

						yielder.r#yield(()).await;
					}
					Ok(None) => break,
					Err(e) => return Err(SignalerError::Receive(e))
				}
			}
			Ok(())
		}))
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalerParseSource {
	ChooseServer,
	SessionInit,
	SessionStream
}

#[derive(Debug)]
pub enum SignalerError<E: RequestExecutor> {
	NoChat,
	Parse { source: SignalerParseSource },
	Deserialize(simd_json::Error),
	Client(ClientError<E::Error>),
	Receive(<E::Response as Response>::Error)
}

impl<E: RequestExecutor> From<simd_json::Error> for SignalerError<E> {
	fn from(e: simd_json::Error) -> Self {
		Self::Deserialize(e)
	}
}
impl<E: RequestExecutor> From<ClientError<E::Error>> for SignalerError<E> {
	fn from(e: ClientError<E::Error>) -> Self {
		Self::Client(e)
	}
}

impl<E: RequestExecutor> fmt::Display for SignalerError<E> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NoChat => f.write_str("stream has no chat"),
			Self::Deserialize(e) => f.write_fmt(format_args!("failed to deserialize response: {e}")),
			Self::Parse { source } => f.write_fmt(format_args!("couldn't parse response from {source:?}")),
			Self::Client(e) => fmt::Display::fmt(e, f),
			Self::Receive(e) => f.write_fmt(format_args!("failed to receive response: {e}"))
		}
	}
}

impl<E: RequestExecutor + fmt::Debug> StdError for SignalerError<E>
where
	E::Response: fmt::Debug
{
	fn cause(&self) -> Option<&dyn StdError> {
		match self {
			Self::Deserialize(e) => Some(e),
			Self::Client(e) => Some(e),
			Self::Receive(e) => Some(e),
			_ => None
		}
	}
}
