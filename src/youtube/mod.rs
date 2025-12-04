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

use std::{error::Error as StdError, fmt, pin::Pin, task::Poll};

use async_stream_lite::try_async_stream;
use futures_util::{Stream, StreamExt, pin_mut, stream::BoxStream};
use pin_project_lite::pin_project;

mod client;
mod context;
mod signaler;
mod types;
mod util;

use self::{
	client::ResponseExt,
	signaler::SignalerChannel,
	types::get_live_chat::{Continuation, GetLiveChatRequest, GetLiveChatResponse},
	util::{TANGO_API_KEY, stringify_runs}
};
pub use self::{
	client::{Client, ClientError, InnertubeError, RequestExecutor, Response},
	context::{StreamChatMode, StreamContext},
	types::{
		ImageContainer, LocalizedRun, LocalizedText, Thumbnail, UnlocalizedText,
		get_live_chat::{Action, ChatItem, MessageRendererBase}
	},
	util::query_channel
};
use crate::youtube::signaler::SignalerError;

#[derive(Debug)]
pub enum ChatEvent {
	Message { text: String }
}

impl ChatEvent {
	pub(crate) fn from_action(action: &Action<'_>) -> Option<Self> {
		match action {
			Action::AddChatItem { item, .. } => match item {
				ChatItem::TextMessage { message_renderer_base, message } => message.as_ref().map(|x| ChatEvent::Message { text: stringify_runs(&x.runs) }),
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
					.unwrap_or_default()
					.into_iter()
					.filter_map(|act| ChatEvent::from_action(&act.action))
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
								.unwrap_or_default()
								.into_iter()
								.filter_map(|act| ChatEvent::from_action(&act.action))
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
									.unwrap_or_default()
									.into_iter()
									.filter_map(|act| ChatEvent::from_action(&act.action))
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
					.unwrap_or_default()
					.into_iter()
					.filter_map(|act| ChatEvent::from_action(&act.action))
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

							for action in contents.live_chat_continuation.actions.unwrap_or_default() {
								if let Action::ReplayChat { actions, .. } = action.action {
									events.extend(actions.into_iter().filter_map(|act| ChatEvent::from_action(&act.action)));
								}
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
			Continuation::Timed { .. } => unimplemented!("Continuation::Timed"),
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
