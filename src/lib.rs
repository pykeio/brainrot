use std::{
	collections::HashMap,
	num::{NonZeroU16, NonZeroU32},
	pin::Pin,
	task::{Context, Poll}
};

use chrono::{DateTime, TimeZone, Utc};
use futures_util::{Stream, StreamExt};
use irc::{
	client::{prelude::Config, Client, ClientStream},
	proto::{Capability, Command, Response}
};
use uuid::Uuid;

const TWITCH_SECURE_IRC: (&str, u16) = ("irc.chat.twitch.tv", 6697);
const TWITCH_CAPABILITY_TAGS: Capability = Capability::Custom("twitch.tv/tags");
const TWITCH_CAPABILITY_MEMBERSHIP: Capability = Capability::Custom("twitch.tv/membership");
const TWITCH_CAPABILITY_COMMANDS: Capability = Capability::Custom("twitch.tv/commands");

pub trait TwitchIdentify {
	fn identify(&self) -> (&str, Option<&str>);
}

pub struct Anonymous;

impl TwitchIdentify for Anonymous {
	fn identify(&self) -> (&str, Option<&str>) {
		("justinfan24340", None)
	}
}

pub struct Authenticated<'u, 'p>(pub &'u str, pub &'p str);

impl<'u, 'p> TwitchIdentify for Authenticated<'u, 'p> {
	fn identify(&self) -> (&str, Option<&str>) {
		(self.0, Some(self.1))
	}
}

#[derive(Debug)]
pub struct Chat {
	stream: ClientStream
}

impl Chat {
	pub async fn new(channel: impl AsRef<str>, auth: impl TwitchIdentify) -> irc::error::Result<Self> {
		let (username, password) = auth.identify();
		let mut client = Client::from_config(Config {
			server: Some(TWITCH_SECURE_IRC.0.to_string()),
			port: Some(TWITCH_SECURE_IRC.1),
			nickname: Some(username.to_string()),
			password: password.map(|c| c.to_string()),
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
			Poll::Ready(Some(Ok(r))) => match crate::chat_event(r) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UserRole {
	Normal,
	Broadcaster,
	Moderator,
	GlobalModerator,
	TwitchAdmin,
	TwitchStaff
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct User {
	pub username: String,
	pub display_name: String,
	pub id: u64,
	pub display_color: Option<u32>,
	pub sub_months: Option<NonZeroU16>,
	pub role: UserRole,
	pub returning_chatter: bool
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "t"))]
pub enum MessageSegment {
	Text { text: String },
	Emote { name: String, id: String }
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

trait MapNonempty {
	type T;
	fn and_then_nonempty<B, F>(self, f: F) -> Option<B>
	where
		Self: Sized,
		F: FnOnce(Self::T) -> Option<B>;
}

impl MapNonempty for String {
	type T = String;
	fn and_then_nonempty<B, F>(self, f: F) -> Option<B>
	where
		Self: Sized,
		F: FnOnce(Self::T) -> Option<B>
	{
		if self.is_empty() { None } else { f(self) }
	}
}

impl MapNonempty for Option<String> {
	type T = String;
	fn and_then_nonempty<B, F>(self, f: F) -> Option<B>
	where
		Self: Sized,
		F: FnOnce(Self::T) -> Option<B>
	{
		self.and_then(|c| c.and_then_nonempty(f))
	}
}

fn get_utf8_slice(s: &str, start: usize, end: usize) -> Option<&str> {
	let mut iter = s.char_indices().map(|(pos, _)| pos).chain(Some(s.len())).skip(start).peekable();
	let start_pos = *iter.peek()?;
	for _ in start..end {
		iter.next();
	}
	Some(&s[start_pos..*iter.peek()?])
}

pub fn chat_event(message: irc::proto::Message) -> Option<ChatEvent> {
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
