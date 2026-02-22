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

use std::{env::args, fmt::Write};

use brainrot::youtube::{self, ChatEvent, MembershipRedemption, RequestExecutor, Run, StreamChatMode, StreamContext, StreamStatus};
use futures_util::StreamExt;

#[derive(Debug, Default)]
struct ReqwestExecutor(reqwest::Client);

impl RequestExecutor for ReqwestExecutor {
	type Response = ReqwestResponse;
	type Error = reqwest::Error;

	async fn make_request(&self, req: http::Request<bytes::Bytes>) -> Result<Self::Response, Self::Error> {
		self.0.execute(req.try_into().unwrap()).await.map(ReqwestResponse)
	}

	async fn sleep(dur: std::time::Duration) {
		tokio::time::sleep(dur).await
	}
}

#[derive(Debug)]
struct ReqwestResponse(reqwest::Response);

impl youtube::Response for ReqwestResponse {
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
	let Some(channel_id) = args().nth(1) else {
		anyhow::bail!("cargo run --example youtube -- <channel_id>");
	};

	let client = youtube::Client::<ReqwestExecutor>::default();
	let streams = youtube::query_channel(&channel_id, &client).await?;

	let Some(stream) = streams.iter().find(|stream| stream.status() == StreamStatus::Live) else {
		eprintln!("Channel has no live streams right now");
		return Ok(());
	};

	println!("Viewing {} (https://www.youtube.com/watch?v={})", stream.title(), stream.id());
	println!("{}", "=".repeat(80));

	let context = StreamContext::new(client, stream.id(), StreamChatMode::Live).await?;
	let mut chat = youtube::Chat::new(context).await?;

	for event in chat.initial_events() {
		print_event(event);
	}

	while let Some(event) = chat.next().await.transpose()? {
		print_event(event);
	}

	Ok(())
}

fn stringify_runs(runs: &[Run]) -> String {
	let mut s = String::new();
	for run in runs {
		write!(&mut s, "{run}").unwrap();
	}
	s
}

fn print_event(event: ChatEvent) {
	match event {
		ChatEvent::Message { author, contents, superchat, .. } => {
			let text = stringify_runs(&contents);
			if let Some(superchat) = superchat {
				println!("{} sent {}: {}", author.name.unwrap_or(author.id), superchat.amount, text);
			} else {
				println!("{}: {}", author.name.unwrap_or(author.id), text);
			}
		}
		ChatEvent::Membership { user, contents, redemption_type, .. } => {
			println!(
				"Membership for {}: {}{}",
				user.name.unwrap_or(user.id),
				stringify_runs(&contents),
				if redemption_type == MembershipRedemption::Gift { " (gifted)" } else { "" }
			);
		}
		ChatEvent::MembershipGift { gifter, contents, .. } => {
			println!("{} gifted memberships: {}", gifter.name.unwrap_or(gifter.id), stringify_runs(&contents));
		}
	}
}
