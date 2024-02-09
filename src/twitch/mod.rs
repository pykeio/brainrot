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

use std::{
	pin::Pin,
	task::{Context, Poll}
};

use futures_util::{Stream, StreamExt};
use irc::{
	client::{prelude::Config, Client, ClientStream},
	proto::Capability
};

pub mod identity;
pub use self::identity::{Anonymous, Authenticated, TwitchIdentity};
mod event;
pub use self::event::{ChatEvent, MessageSegment, User, UserRole};

const TWITCH_SECURE_IRC: (&str, u16) = ("irc.chat.twitch.tv", 6697);
const TWITCH_CAPABILITY_TAGS: Capability = Capability::Custom("twitch.tv/tags");
const TWITCH_CAPABILITY_MEMBERSHIP: Capability = Capability::Custom("twitch.tv/membership");
const TWITCH_CAPABILITY_COMMANDS: Capability = Capability::Custom("twitch.tv/commands");

/// A connection to a Twitch IRC channel.
///
/// In order for the connection to stay alive, the IRC client must be able to receive and respond to ping messages, thus
/// you must poll the stream for as long as you wish the client to stay alive. If that isn't possible, start a dedicated
/// thread for the client and send chat events back to your application over an `mpsc` or other channel.
#[derive(Debug)]
pub struct Chat {
	stream: ClientStream
}

impl Chat {
	/// Connect to a Twitch IRC channel.
	///
	/// ```no_run
	/// use brainrot::twitch::{Anonymous, Chat};
	///
	/// # #[tokio::main]
	/// # async fn main() -> anyhow::Result<()> {
	/// let mut client = Chat::new("miyukiwei", Anonymous).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(channel: impl AsRef<str>, auth: impl TwitchIdentity) -> irc::error::Result<Self> {
		let (username, password) = auth.as_identity();
		let mut client = Client::from_config(Config {
			server: Some(TWITCH_SECURE_IRC.0.to_string()),
			port: Some(TWITCH_SECURE_IRC.1),
			nickname: Some(username.to_string()),
			password: password.map(|c| format!("oauth:{c}")),
			channels: vec![format!("#{}", channel.as_ref())],
			..Default::default()
		})
		.await?;
		client.send_cap_req(&[TWITCH_CAPABILITY_COMMANDS, TWITCH_CAPABILITY_MEMBERSHIP, TWITCH_CAPABILITY_TAGS])?;
		client.identify()?;
		Ok(Self { stream: client.stream()? })
	}
}

impl Stream for Chat {
	type Item = irc::error::Result<ChatEvent>;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		let next = self.stream.poll_next_unpin(cx);
		match next {
			Poll::Ready(Some(Ok(r))) => match self::event::to_chat_event(r) {
				Some(ev) => Poll::Ready(Some(Ok(ev))),
				None => {
					cx.waker().wake_by_ref();
					Poll::Pending
				}
			},
			Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
			Poll::Ready(None) => Poll::Ready(None),
			Poll::Pending => Poll::Pending
		}
	}
}
