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

use brainrot::youtube::{self, Action, ChatItem};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let context =
		youtube::ChatContext::new_from_channel(args().nth(1).as_deref().unwrap_or("@miyukiwei"), youtube::ChannelSearchOptions::LatestLiveOrUpcoming).await?;
	let mut stream = youtube::stream(&context).await?;
	while let Some(Ok(c)) = stream.next().await {
		if let Action::AddChatItem {
			item: ChatItem::TextMessage { message_renderer_base, message },
			..
		} = c
		{
			println!(
				"{}: {}",
				message_renderer_base.author_name.unwrap_or_default().simple_text,
				message.unwrap().runs.into_iter().map(|c| c.to_chat_string()).collect::<String>()
			);
		}
	}
	Ok(())
}
