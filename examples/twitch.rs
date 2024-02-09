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

use brainrot::{twitch, TwitchChat, TwitchChatEvent};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let mut client = TwitchChat::new(args().nth(1).as_deref().unwrap_or("miyukiwei"), twitch::Anonymous).await?;

	while let Some(message) = client.next().await.transpose()? {
		if let TwitchChatEvent::Message { user, contents, .. } = message {
			println!("{}: {}", user.display_name, contents.iter().map(|c| c.to_string()).collect::<String>());
		}
	}

	Ok(())
}
