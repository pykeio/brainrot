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

use std::{collections::HashMap, io::BufRead, iter};

use rand::Rng;
use reqwest::{Response, header};
use simd_json::{
	OwnedValue,
	base::{ValueAsArray, ValueAsScalar}
};
use url::Url;

use super::{Error, util::SimdJsonResponseBody};

const GCM_SIGNALER_SRQE: &str = "https://signaler-pa.youtube.com/punctual/v1/chooseServer";
const GCM_SIGNALER_PSUB: &str = "https://signaler-pa.youtube.com/punctual/multi-watch/channel";

#[derive(Debug, Default)]
pub struct SignalerChannelInner {
	pub(crate) topic: String,
	tango_key: String,
	gsessionid: Option<String>,
	sid: Option<String>,
	rid: usize,
	pub(crate) aid: usize,
	session_n: usize
}

impl SignalerChannelInner {
	pub fn with_topic(topic: impl ToString, tango_key: impl ToString) -> Self {
		Self {
			topic: topic.to_string(),
			tango_key: tango_key.to_string(),
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
		let mut rng = rand::rng();
		iter::repeat_with(|| CHARSET[rng.random_range(0..CHARSET.len())] as char)
			.take(11)
			.collect()
	}

	pub async fn choose_server(&mut self) -> Result<(), Error> {
		let server_response: OwnedValue = super::get_http_client()
			.post(Url::parse_with_params(GCM_SIGNALER_SRQE, [("key", &self.tango_key)])?)
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

	pub async fn init_session(&mut self) -> Result<(), Error> {
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
					("key", &self.tango_key),
					("RID", &self.rid.to_string()),
					("AID", &self.aid.to_string()),
					("CVER", "22"),
					("zx", Self::gen_zx().as_ref()),
					("t", "1")
				]
			)?)
			// yes, this is required. why? who the fuck knows! but if you don't provide this, you get the typical google
			// robot error complaining about an invalid request body when you GET GCM_SIGNALER_PSUB. yes, invalid request
			// body, in a GET request. where the error actually refers to this POST request. because that makes sense.
			.header("X-WebChannel-Content-Type", "application/json+protobuf")
			.form(&ofs_parameters)
			.send()
			.await?;

		let mut ofs_res_line = ofs.bytes().await?.lines().nth(1).unwrap().unwrap();
		let value: OwnedValue = unsafe { simd_json::from_str(&mut ofs_res_line) }?;
		let value = value.as_array().unwrap()[0].as_array().unwrap();
		// first value might be 1 if the request has an error, not entirely sure
		assert_eq!(value[0].as_usize().unwrap(), 0);
		let sid = value[1].as_array().unwrap()[1].as_str().unwrap();
		self.sid = Some(sid.to_owned());
		Ok(())
	}

	pub async fn get_session_stream(&self) -> Result<Response, Error> {
		Ok(super::get_http_client()
			.get(Url::parse_with_params(GCM_SIGNALER_PSUB, [
				("VER", "8"),
				("gsessionid", self.gsessionid.as_ref().unwrap()),
				("key", &self.tango_key),
				("RID", "rpc"),
				("SID", self.sid.as_ref().unwrap()),
				("AID", &self.aid.to_string()),
				("CI", "0"),
				("TYPE", "xmlhttp"),
				("zx", &Self::gen_zx()),
				("t", "1")
			])?)
			.header(header::CONNECTION, "keep-alive")
			.send()
			.await?)
	}
}
