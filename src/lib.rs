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

#[cfg(feature = "twitch")]
pub mod twitch;
#[cfg(feature = "twitch")]
pub use self::twitch::{Chat as TwitchChat, ChatEvent as TwitchChatEvent, MessageSegment as TwitchMessageSegment, TwitchIdentity};

#[cfg(feature = "youtube")]
pub mod youtube;

pub(crate) mod util;
