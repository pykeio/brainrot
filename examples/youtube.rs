// Copyright 2024 pyke.io
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env::args;

use brainrot::youtube::{self, ChatEvent, RequestExecutor, Response, StreamChatMode, StreamContext};
use futures_util::StreamExt;

#[derive(Debug, Default)]
struct ReqwestExecutor(reqwest::Client);

impl RequestExecutor for ReqwestExecutor {
	type Response = Respownse;
	type Error = reqwest::Error;

	async fn make_request(&self, req: http::Request<bytes::Bytes>) -> Result<Self::Response, Self::Error> {
		self.0.execute(req.try_into().unwrap()).await.map(Respownse)
	}
}

#[derive(Debug)]
struct Respownse(reqwest::Response);

impl Response for Respownse {
	type Error = reqwest::Error;

	fn status_code(&self) -> u16 {
		self.0.status().as_u16()
	}

	async fn recv_chunk(&mut self) -> Result<Option<bytes::Bytes>, Self::Error> {
		self.0.chunk().await
	}
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let client = youtube::Client::<ReqwestExecutor>::default();
	let streams = youtube::query_channel(args().nth(1).as_deref().unwrap_or("@miyukiwei"), &client).await?;

	let context = StreamContext::new(client, streams[0].id(), StreamChatMode::Live).await?;
	let mut chat = youtube::Chat::new(context).await?;

	for event in chat.initial_events() {
		match event {
			ChatEvent::Message { text } => println!("{text}")
		}
	}

	while let Some(event) = chat.next().await.transpose()? {
		match event {
			ChatEvent::Message { text } => println!("{text}")
		}
	}

	Ok(())
}
