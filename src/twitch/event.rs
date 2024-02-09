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
	collections::HashMap,
	num::{NonZeroU16, NonZeroU32}
};

use chrono::{DateTime, TimeZone, Utc};
use irc::proto::{Command, Response};
use uuid::Uuid;

use crate::util::{get_utf8_slice, MapNonempty};

/// A user's role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UserRole {
	Normal,
	/// User is the one broadcasting.
	Broadcaster,
	/// User is a moderator for this channel.
	Moderator,
	/// User is a Twitch 'global' moderator.
	GlobalModerator,
	TwitchAdmin,
	TwitchStaff
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct User {
	pub username: String,
	/// The user's display name. This is almost always identical to the username, just with different capitalization.
	///
	/// Though I vaguely remember seeing a display name written with CJK characters once or twice, so make sure your
	/// application is equipped to handle that should it arise.
	pub display_name: String,
	/// The user's channel ID, for use via the Twitch API.
	pub id: u64,
	/// The user's preferred display color if they have one set.
	pub display_color: Option<u32>,
	/// If the user is subscribed to the broadcasting channel, describes how many months the user has been subscribed.
	pub sub_months: Option<NonZeroU16>,
	/// The user's role.
	pub role: UserRole,
	/// Whether or not the user is a "returning" chatter in this channel.
	pub returning_chatter: bool
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "t"))]
pub enum MessageSegment {
	Text {
		text: String
	},
	/// The emote URL can be accessed via `https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_{id}/default/{color}/{size}`, where:
	/// - `id` is the `id` field in this variant.
	/// - `color` is either `dark` or `light`, referring to the background color of the element the emote will be
	///   displayed in.
	/// - `size` is either `1.0`, `2.0,` or `3.0` with 1.0 being the smallest (for inline display) and 3.0 being the
	///   largest.
	///
	/// This URL typically returns either a GIF (for animated emotes) or PNG (for static emotes); check the returned
	/// `Content-Type` header.
	Emote {
		name: String,
		id: String
	}
}

impl ToString for MessageSegment {
	fn to_string(&self) -> String {
		match self {
			Self::Text { text } => text.to_owned(),
			Self::Emote { name, .. } => name.to_owned()
		}
	}
}

#[derive(Debug)]
pub enum ChatEvent {
	Message {
		id: Uuid,
		user: User,
		sent_at: DateTime<Utc>,
		reply_to: Option<Uuid>,
		emote_only: bool,
		first_message: bool,
		contents: Vec<MessageSegment>
	},
	SendBits {
		id: Uuid,
		user: User,
		bits: NonZeroU32,
		sent_at: DateTime<Utc>,
		segments: Vec<MessageSegment>
	},
	MemberChunk {
		names: Vec<String>
	},
	EndOfMembers
}

pub(crate) fn to_chat_event(message: irc::proto::Message) -> Option<ChatEvent> {
	match message.command {
		Command::PRIVMSG(_, msg) => {
			let mut tags = message
				.tags?
				.into_iter()
				.filter(|c| c.1.is_some())
				.map(|c| (c.0, c.1.unwrap()))
				.collect::<HashMap<_, _>>();

			let (username, user_display_name) = match message.prefix? {
				irc::proto::Prefix::Nickname(n1, n2, _) => (
					n1,
					match tags.remove("display-name") {
						Some(display_name) => {
							if display_name.is_empty() {
								n2
							} else {
								display_name
							}
						}
						None => n2
					}
				),
				_ => return None
			};

			let mut badges = tags
				.remove("badges")
				.and_then_nonempty(|c| {
					c.split(',')
						.map(|f| {
							let mut split = f.splitn(2, '/');
							Some((split.next()?.to_owned(), split.next()?.to_owned()))
						})
						.collect::<Option<HashMap<_, _>>>()
				})
				.unwrap_or_default();
			let mut badge_info = tags
				.remove("badge-info")
				.and_then_nonempty(|c| {
					c.split(',')
						.map(|f| {
							let mut split = f.splitn(2, '/');
							Some((split.next()?.to_owned(), split.next()?.to_owned()))
						})
						.collect::<Option<HashMap<_, _>>>()
				})
				.unwrap_or_default();

			let color = tags.remove("color").and_then_nonempty(|c| u32::from_str_radix(&c[1..], 16).ok());

			let mut emotes = vec![];
			for emote in tags.remove("emotes")?.split('/') {
				if emote.is_empty() {
					break;
				}

				let mut split = emote.splitn(2, ':');
				let (id, ranges) = (split.next()?, split.next()?);
				for range in ranges.split(',') {
					let mut split = range.splitn(2, '-');
					let (from, to) = (split.next().and_then(|f| f.parse::<usize>().ok())?, split.next().and_then(|f| f.parse::<usize>().ok())?);
					emotes.push((id.to_owned(), from, to));
				}
			}
			emotes.sort_by(|a, b| a.1.cmp(&b.1));

			let mut segments = Vec::with_capacity(emotes.len());
			if !emotes.is_empty() {
				let mut i = 0;
				for (id, start, end) in emotes {
					if start > i {
						segments.push(MessageSegment::Text {
							text: get_utf8_slice(&msg, i, start)?.to_owned()
						});
					}
					if end >= start {
						segments.push(MessageSegment::Emote {
							name: get_utf8_slice(&msg, start, end + 1)?.to_owned(),
							id
						});
						i = end + 1;
					}
				}
				if i < msg.len() {
					segments.push(MessageSegment::Text {
						text: get_utf8_slice(&msg, i, msg.len())?.to_string()
					});
				}
			} else {
				segments.push(MessageSegment::Text { text: msg });
			}

			let user = User {
				username,
				display_name: user_display_name,
				display_color: color,
				role: match tags.remove("user-type").as_deref() {
					Some("admin") => UserRole::TwitchAdmin,
					Some("global_mod") => UserRole::GlobalModerator,
					Some("staff") => UserRole::TwitchStaff,
					_ => match tags.remove("mod").as_deref() {
						Some("1") => UserRole::Moderator,
						_ => match badges.remove("broadcaster").as_deref() {
							Some(_) => UserRole::Broadcaster,
							_ => UserRole::Normal
						}
					}
				},
				returning_chatter: matches!(tags.remove("returning-chatter").as_deref(), Some("1")),
				sub_months: badge_info.remove("subscriber").and_then(|f| f.parse().ok()),
				id: tags.remove("user-id").and_then(|f| f.parse().ok())?
			};

			let id = tags.remove("id").and_then(|f| f.parse().ok())?;
			let sent_at = Utc
				.timestamp_opt(tags.remove("tmi-sent-ts").and_then(|f| f.parse::<i64>().map(|f| f / 1000).ok())?, 0)
				.latest()?;

			if let Some(bits) = tags.remove("bits").and_then_nonempty(|f| f.parse().ok()) {
				return Some(ChatEvent::SendBits { id, user, bits, sent_at, segments });
			}

			Some(ChatEvent::Message {
				id,
				user,
				reply_to: tags.remove("reply-parent-msg-id").and_then(|f| f.parse().ok()),
				sent_at,
				emote_only: matches!(tags.remove("emote-only").as_deref(), Some("1")),
				first_message: matches!(tags.remove("first-msg").as_deref(), Some("1")),
				contents: segments
			})
		}
		Command::Response(Response::RPL_NAMREPLY, names) => Some(ChatEvent::MemberChunk { names: names[3..].to_vec() }),
		Command::Response(Response::RPL_ENDOFNAMES, _) => Some(ChatEvent::EndOfMembers),
		_ => None
	}
}
