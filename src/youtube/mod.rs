// Copyright 2024 pyke.io
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

use std::{error::Error as StdError, fmt, pin::Pin, task::Poll, time::Duration};

use async_stream_lite::try_async_stream;
use futures_util::{Stream, StreamExt, pin_mut, stream::BoxStream};
use pin_project_lite::pin_project;
use simd_json::{BorrowedValue, derived::ValueTryAsObject};

mod client;
mod context;
mod signaler;
mod types;
mod util;

use self::{
	client::ResponseExt,
	signaler::{SignalerChannel, SignalerError},
	types::get_live_chat::{ChatItemHeader, Continuation, GetLiveChatRequest, GetLiveChatResponse},
	util::TANGO_API_KEY
};
pub use self::{
	client::{Client, ClientError, InnertubeError, RequestExecutor, Response},
	context::{StreamChatMode, StreamContext},
	types::{
		ImageContainer, LocalizedRun, LocalizedText, Thumbnail, UnlocalizedText,
		get_live_chat::{Action, ChatItem, MessageRendererBase}
	},
	util::{ChannelStream, QueryChannelError, StreamStatus, query_channel}
};

#[derive(Debug, Clone)]
pub struct Image {
	pub url: String,
	pub size: Option<(u32, u32)>
}

#[derive(Debug, Clone)]
pub struct AuthorBadge {
	pub name: String,
	pub icon: Vec<Image>,
	pub icon_type: Option<String>
}

#[derive(Debug, Clone)]
pub struct Author {
	pub id: String,
	pub name: Option<String>,
	pub avatars: Vec<Image>,
	pub badges: Vec<AuthorBadge>
}

impl Author {
	pub(crate) fn from_message_base(base: &types::get_live_chat::MessageRendererBase) -> Self {
		Author {
			id: base.author_external_channel_id.to_string(),
			name: base.author_name.as_ref().map(|text| text.simple_text.to_string()),
			avatars: base
				.author_photo
				.thumbnails
				.iter()
				.map(|thumb| Image {
					url: thumb.url.to_string(),
					size: match (thumb.width, thumb.height) {
						(Some(width), Some(height)) => Some((width as u32, height as u32)),
						_ => None
					}
				})
				.collect(),
			badges: base
				.author_badges
				.iter()
				.map(|badge| AuthorBadge {
					name: badge.live_chat_author_badge_renderer.tooltip.to_string(),
					icon_type: badge.live_chat_author_badge_renderer.icon.as_ref().map(|icon| icon.icon_type.to_string()),
					icon: badge
						.live_chat_author_badge_renderer
						.custom_thumbnail
						.as_ref()
						.map(|img| {
							img.thumbnails
								.iter()
								.map(|thumb| Image {
									url: thumb.url.to_string(),
									size: match (thumb.width, thumb.height) {
										(Some(width), Some(height)) => Some((width as u32, height as u32)),
										_ => None
									}
								})
								.collect()
						})
						.unwrap_or_default()
				})
				.collect()
		}
	}
}

#[derive(Debug, Clone)]
pub struct SuperchatMeta {
	pub amount: String,
	pub header_background_color: u32,
	pub header_text_color: u32,
	pub body_background_color: u32,
	pub body_text_color: u32,
	pub author_name_text_color: u32
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembershipRedemption {
	/// Membership was purchased by user
	Purchase,
	/// Membership was redeemed from a gift
	Gift
}

#[derive(Debug, Clone)]
pub enum Run {
	Text(String),
	Emoji { name: String, id: String, images: Vec<Image> }
}

impl Run {
	pub(crate) fn from_localized_run(run: &LocalizedRun) -> Self {
		match run {
			LocalizedRun::Text { text } => Run::Text(text.to_string()),
			LocalizedRun::Emoji { emoji, .. } => {
				let label = emoji
					.image
					.accessibility
					.as_ref()
					.expect("emojis should always have accessibility data")
					.accessibility_data
					.label
					.to_string();
				if emoji.is_custom_emoji {
					Run::Emoji {
						name: label,
						id: emoji.emoji_id.to_string(),
						images: emoji
							.image
							.thumbnails
							.iter()
							.map(|thumb| Image {
								url: thumb.url.to_string(),
								size: match (thumb.width, thumb.height) {
									(Some(width), Some(height)) => Some((width as u32, height as u32)),
									_ => None
								}
							})
							.collect()
					}
				} else {
					Run::Text(label)
				}
			}
		}
	}
}

impl fmt::Display for Run {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Run::Text(text) => f.write_str(text),
			Run::Emoji { name, .. } => f.write_fmt(format_args!(":{name}:"))
		}
	}
}

#[derive(Debug, Clone)]
pub enum ChatEvent {
	Message {
		id: String,
		author: Author,
		contents: Vec<Run>,
		timestamp_ms: i64,
		superchat: Option<SuperchatMeta>
	},
	Membership {
		id: String,
		user: Author,
		contents: Vec<Run>,
		timestamp_ms: i64,
		redemption_type: MembershipRedemption
	},
	MembershipGift {
		id: String,
		gifter: Author,
		contents: Vec<Run>,
		timestamp_ms: i64
	}
}

impl ChatEvent {
	pub(crate) fn from_action(action: BorrowedValue<'_>) -> Option<Self> {
		let Ok(action) = simd_json::serde::from_refborrowed_value(&action) else {
			let action_key = action.try_as_object().ok().and_then(|c| c.keys().next())?;
			tracing::warn!("Encountered unknown or malformed action `{action_key}`");
			tracing::trace!("bad action: {}", simd_json::to_string(&action).as_deref().unwrap_or("<STRINGIFY ERR>"));
			return None;
		};

		match action {
			Action::AddChatItem { item, .. } => match item {
				ChatItem::TextMessage { base, message } => Some(ChatEvent::Message {
					id: base.id.to_string(),
					author: Author::from_message_base(&base),
					contents: message
						.as_ref()
						.map(|text| text.runs.iter().map(Run::from_localized_run).collect())
						.unwrap_or_default(),
					timestamp_ms: base.timestamp_usec / 1000,
					superchat: None
				}),
				ChatItem::Superchat {
					base,
					message,
					purchase_amount_text,
					header_background_color,
					header_text_color,
					body_background_color,
					body_text_color,
					author_name_text_color
				} => Some(ChatEvent::Message {
					id: base.id.to_string(),
					author: Author::from_message_base(&base),
					contents: message
						.as_ref()
						.map(|text| text.runs.iter().map(Run::from_localized_run).collect())
						.unwrap_or_default(),
					timestamp_ms: base.timestamp_usec / 1000,
					superchat: Some(SuperchatMeta {
						amount: purchase_amount_text.simple_text.to_string(),
						header_background_color: header_background_color as _,
						author_name_text_color: author_name_text_color as _,
						body_background_color: body_background_color as _,
						body_text_color: body_text_color as _,
						header_text_color: header_text_color as _
					})
				}),
				ChatItem::MembershipItem { base, header_sub_text } => Some(ChatEvent::Membership {
					id: base.id.to_string(),
					user: Author::from_message_base(&base),
					contents: header_sub_text
						.as_ref()
						.map(|text| text.runs.iter().map(Run::from_localized_run).collect())
						.unwrap_or_default(),
					timestamp_ms: base.timestamp_usec / 1000,
					redemption_type: MembershipRedemption::Purchase
				}),
				ChatItem::MembershipGiftRedemption { base, message } => Some(ChatEvent::Membership {
					id: base.id.to_string(),
					user: Author::from_message_base(&base),
					contents: message
						.as_ref()
						.map(|text| text.runs.iter().map(Run::from_localized_run).collect())
						.unwrap_or_default(),
					timestamp_ms: base.timestamp_usec / 1000,
					redemption_type: MembershipRedemption::Gift
				}),
				ChatItem::MembershipGift {
					id,
					timestamp_usec,
					author_external_channel_id,
					header
				} => match header {
					ChatItemHeader::Sponsorship {
						author_name,
						author_photo,
						author_badges,
						primary_text
					} => Some(ChatEvent::MembershipGift {
						id: id.to_string(),
						gifter: Author::from_message_base(&MessageRendererBase {
							id: author_external_channel_id,
							author_name,
							author_photo,
							author_badges,
							timestamp_usec,
							author_external_channel_id
						}),
						contents: primary_text.runs.iter().map(Run::from_localized_run).collect(),
						timestamp_ms: timestamp_usec / 1000
					})
				},
				_ => None
			},
			Action::ReplayChat { .. } => unreachable!("ReplayChat should be collapsed"),
			_ => None
		}
	}
}

pin_project! {
	pub struct Chat<E: RequestExecutor> {
		initial_events: Vec<ChatEvent>,
		#[pin]
		stream: BoxStream<'static, Result<ChatEvent, ChatError<E>>>
	}
}

impl<E: RequestExecutor> Chat<E> {
	pub async fn new(context: StreamContext<E>) -> Result<Self, ChatError<E>> {
		let mut initial_continuation_bytes = if !context.is_replay {
			context
				.client
				.chat_live(GetLiveChatRequest {
					continuation: &context.initial_continuation
				})
				.await?
		} else {
			context
				.client
				.chat_replay(GetLiveChatRequest {
					continuation: &context.initial_continuation
				})
				.await?
		}
		.with_innertube_error()
		.await?
		.recv_all()
		.await
		.map_err(ChatError::Receive)?;
		let initial_continuation: GetLiveChatResponse<'_> = simd_json::from_slice(&mut initial_continuation_bytes)?;

		let Some(contents) = initial_continuation.continuation_contents else {
			return Err(ChatError::NoChat);
		};

		match &contents.live_chat_continuation.continuations[0] {
			Continuation::Invalidation { invalidation_id, continuation, .. } => {
				let continuation_token = continuation.to_string();

				let mut channel = SignalerChannel::with_topic(invalidation_id.topic, TANGO_API_KEY);

				let initial_events = contents
					.live_chat_continuation
					.actions
					.into_iter()
					.filter_map(|act| ChatEvent::from_action(act.action))
					.collect();
				let _ = initial_continuation;
				let _ = initial_continuation_bytes;

				Ok(Self {
					initial_events,
					stream: Box::pin(try_async_stream(move |yielder| async move {
						let mut continuation_token = continuation_token;
						'i: loop {
							let mut continuation_bytes = context
								.client
								.chat_live(GetLiveChatRequest { continuation: &continuation_token })
								.await?
								.with_innertube_error()
								.await?
								.recv_all()
								.await
								.map_err(ChatError::Receive)?;
							let continuation: GetLiveChatResponse<'_> = simd_json::from_slice(&mut continuation_bytes)?;
							let Some(contents) = continuation.continuation_contents else {
								break;
							};

							for event in contents
								.live_chat_continuation
								.actions
								.into_iter()
								.filter_map(|act| ChatEvent::from_action(act.action))
							{
								yielder.r#yield(event).await;
							}

							let Some(Continuation::Invalidation { continuation: next_token, .. }) = contents.live_chat_continuation.continuations.first()
							else {
								break;
							};

							continuation_token.clear();
							continuation_token.push_str(next_token);

							let _ = continuation;
							let _ = continuation_bytes;

							let signaler_stream = channel.stream(&context.client).await?;
							pin_mut!(signaler_stream);
							while let Some(()) = signaler_stream.next().await.transpose()? {
								let mut continuation = context
									.client
									.chat_live(GetLiveChatRequest { continuation: &continuation_token })
									.await?
									.with_innertube_error()
									.await?
									.recv_all()
									.await
									.map_err(ChatError::Receive)?;
								let continuation: GetLiveChatResponse<'_> = simd_json::from_slice(&mut continuation)?;
								let Some(contents) = continuation.continuation_contents else {
									break 'i;
								};

								for event in contents
									.live_chat_continuation
									.actions
									.into_iter()
									.filter_map(|act| ChatEvent::from_action(act.action))
								{
									yielder.r#yield(event).await;
								}

								let Some(Continuation::Invalidation { continuation: next_token, .. }) = contents.live_chat_continuation.continuations.first()
								else {
									break 'i;
								};

								continuation_token.clear();
								continuation_token.push_str(next_token);
							}
						}
						Ok(())
					}))
				})
			}
			Continuation::Replay { continuation, .. } => {
				let continuation_token = continuation.to_string();
				let events: Vec<ChatEvent> = contents
					.live_chat_continuation
					.actions
					.into_iter()
					.filter_map(|act| ChatEvent::from_action(act.action))
					.collect();

				let _ = initial_continuation;
				let _ = initial_continuation_bytes;

				Ok(Self {
					initial_events: Vec::default(),
					stream: Box::pin(try_async_stream(move |yielder| async move {
						let mut continuation_token = continuation_token;
						let mut events = events;
						loop {
							for event in events.drain(..) {
								yielder.r#yield(event).await;
							}

							let mut continuation = context
								.client
								.chat_replay(GetLiveChatRequest { continuation: &continuation_token })
								.await?
								.with_innertube_error()
								.await?
								.recv_all()
								.await
								.map_err(ChatError::Receive)?;
							let continuation: GetLiveChatResponse<'_> = simd_json::from_slice(&mut continuation)?;
							let Some(contents) = continuation.continuation_contents else {
								break;
							};

							for action in contents.live_chat_continuation.actions {
								let Ok(Action::ReplayChat { actions, .. }) = simd_json::serde::from_borrowed_value(action.action) else {
									continue;
								};

								events.extend(actions.into_iter().filter_map(|act| ChatEvent::from_action(act.action)));
							}

							let Some(Continuation::Replay { continuation: next_token, .. }) = contents.live_chat_continuation.continuations.first() else {
								for event in events.drain(..) {
									yielder.r#yield(event).await;
								}
								break;
							};

							continuation_token.clear();
							continuation_token.push_str(next_token);
						}
						Ok(())
					}))
				})
			}
			Continuation::Timed { continuation, timeout_ms } => {
				let continuation_token = continuation.to_string();
				let timeout = Duration::from_millis(*timeout_ms as _);

				let events: Vec<ChatEvent> = contents
					.live_chat_continuation
					.actions
					.into_iter()
					.filter_map(|act| ChatEvent::from_action(act.action))
					.collect();

				let _ = initial_continuation;
				let _ = initial_continuation_bytes;

				Ok(Self {
					initial_events: events,
					stream: Box::pin(try_async_stream(move |yielder| async move {
						let mut continuation_token = continuation_token;
						let mut timeout = timeout;
						loop {
							E::sleep(timeout).await;

							let mut continuation = context
								.client
								.chat_live(GetLiveChatRequest { continuation: &continuation_token })
								.await?
								.with_innertube_error()
								.await?
								.recv_all()
								.await
								.map_err(ChatError::Receive)?;
							let continuation: GetLiveChatResponse<'_> = simd_json::from_slice(&mut continuation)?;
							let Some(contents) = continuation.continuation_contents else {
								break;
							};

							for event in contents
								.live_chat_continuation
								.actions
								.into_iter()
								.filter_map(|act| ChatEvent::from_action(act.action))
							{
								yielder.r#yield(event).await;
							}

							let Some(Continuation::Timed { continuation: next_token, timeout_ms }) = contents.live_chat_continuation.continuations.first()
							else {
								break;
							};

							continuation_token.clear();
							continuation_token.push_str(next_token);

							timeout = Duration::from_millis(*timeout_ms as _);
						}
						Ok(())
					}))
				})
			}
			Continuation::PlayerSeek { .. } => unreachable!("PlayerSeek shouldn't be the first continuation")
		}
	}

	pub fn initial_events(&mut self) -> impl Iterator<Item = ChatEvent> + '_ {
		self.initial_events.drain(..)
	}
}

impl<E: RequestExecutor> Stream for Chat<E> {
	type Item = Result<ChatEvent, ChatError<E>>;

	fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
		self.project().stream.poll_next(cx)
	}
}

#[derive(Debug)]
pub enum ChatError<E: RequestExecutor> {
	NoChat,
	Deserialize(simd_json::Error),
	Client(ClientError<E::Error>),
	Receive(<E::Response as Response>::Error),
	Signaler(SignalerError<E>),
	Innertube(InnertubeError)
}

impl<E: RequestExecutor> From<simd_json::Error> for ChatError<E> {
	fn from(e: simd_json::Error) -> Self {
		Self::Deserialize(e)
	}
}
impl<E: RequestExecutor> From<ClientError<E::Error>> for ChatError<E> {
	fn from(e: ClientError<E::Error>) -> Self {
		Self::Client(e)
	}
}
impl<E: RequestExecutor> From<InnertubeError> for ChatError<E> {
	fn from(e: InnertubeError) -> Self {
		Self::Innertube(e)
	}
}
impl<E: RequestExecutor> From<SignalerError<E>> for ChatError<E> {
	fn from(e: SignalerError<E>) -> Self {
		Self::Signaler(e)
	}
}

impl<E: RequestExecutor> fmt::Display for ChatError<E> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NoChat => f.write_str("stream has no chat"),
			Self::Deserialize(e) => f.write_fmt(format_args!("failed to deserialize response: {e}")),
			Self::Client(e) => fmt::Display::fmt(e, f),
			Self::Receive(e) => f.write_fmt(format_args!("failed to receive response: {e}")),
			Self::Signaler(e) => f.write_fmt(format_args!("real-time channel failed: {e}")),
			Self::Innertube(e) => fmt::Display::fmt(e, f)
		}
	}
}

impl<E: RequestExecutor + fmt::Debug> StdError for ChatError<E>
where
	E::Response: fmt::Debug
{
	fn cause(&self) -> Option<&dyn StdError> {
		match self {
			Self::Deserialize(e) => Some(e),
			Self::Client(e) => Some(e),
			Self::Receive(e) => Some(e),
			Self::Signaler(e) => Some(e),
			Self::Innertube(e) => Some(e),
			_ => None
		}
	}
}
