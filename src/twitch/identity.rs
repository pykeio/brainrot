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

/// Represents a type that can be used to identify the client.
pub trait TwitchIdentity {
	/// Converts this type into a tuple of `(username, Option<auth_key>)`.
	fn as_identity(&self) -> (&str, Option<&str>);
}

/// Anonymous identity with no authentication. It will not show up in the chatters list, and its capabilities are
/// limited. This can really only be used to receive chat events.
#[derive(Debug, Clone, Copy)]
pub struct Anonymous;

impl TwitchIdentity for Anonymous {
	fn as_identity(&self) -> (&str, Option<&str>) {
		("justinfan24340", None)
	}
}

/// Authenticated identity with a username and OAuth access token.
///
/// For more information on OAuth scopes and how to acquire an OAuth token, see the Twitch documentation: <https://dev.twitch.tv/docs/irc/authenticate-bot/>
///
/// Note that the account will show up in the chatters list. If you are indiscriminately crawling stream chats, please
/// use [`Anonymous`] instead.
///
/// ```no_run
/// use brainrot::twitch::{Authenticated, Chat};
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let mut client = Chat::new("miyukiwei", Authenticated("yukifan4", "yfvzjqb705z12hrhy1zkwa9xt7v662")).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Authenticated<'u, 'p>(pub &'u str, pub &'p str);

impl<'u, 'p> TwitchIdentity for Authenticated<'u, 'p> {
	fn as_identity(&self) -> (&str, Option<&str>) {
		(self.0, Some(self.1))
	}
}
