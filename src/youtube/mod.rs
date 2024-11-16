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

use std::{collections::HashSet, io::BufRead, sync::OnceLock, time::Duration};

use async_stream_lite::try_async_stream;
use futures_util::stream::BoxStream;
use reqwest::header::{self, HeaderMap, HeaderValue};
use simd_json::base::{ValueAsContainer, ValueAsScalar};
use thiserror::Error;
use tokio::time::sleep;

mod context;
mod error;
mod signaler;
mod types;
mod util;

pub use self::{
	context::{ChannelSearchOptions, ChatContext, LiveStreamStatus},
	error::Error,
	types::{
		ImageContainer, LocalizedRun, LocalizedText, Thumbnail, UnlocalizedText,
		get_live_chat::{Action, ChatItem, MessageRendererBase}
	}
};
use self::{
	signaler::SignalerChannelInner,
	types::get_live_chat::{Continuation, GetLiveChatResponse}
};

const TANGO_LIVE_ENDPOINT: &str = "https://www.youtube.com/youtubei/v1/live_chat/get_live_chat";
const TANGO_REPLAY_ENDPOINT: &str = "https://www.youtube.com/youtubei/v1/live_chat/get_live_chat_replay";

pub(crate) fn get_http_client() -> &'static reqwest::Client {
	static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
	HTTP_CLIENT.get_or_init(|| {
		let mut headers = HeaderMap::new();
		// Set our Accept-Language to en-US so we can properly match substrings
		headers.append(header::ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
		headers.append(header::USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:123.0) Gecko/20100101 Firefox/123.0"));
		// Referer is required by Signaler endpoints.
		headers.append(header::REFERER, HeaderValue::from_static("https://www.youtube.com/"));
		reqwest::Client::builder().default_headers(headers).build().unwrap()
	})
}

struct ActionChunk<'r> {
	actions: Vec<Action>,
	ctx: &'r ChatContext,
	continuation_token: Option<String>,
	pub(crate) signaler_topic: Option<String>
}

unsafe impl<'r> Send for ActionChunk<'r> {}

impl<'r> ActionChunk<'r> {
	pub fn new(response: GetLiveChatResponse, ctx: &'r ChatContext) -> Result<Self, Error> {
		let continuation_contents = response.continuation_contents.ok_or(Error::EndOfContinuation)?;

		let continuation_token = match &continuation_contents.live_chat_continuation.continuations[0] {
			Continuation::Invalidation { continuation, .. } => continuation.to_owned(),
			Continuation::Timed { continuation, .. } => continuation.to_owned(),
			Continuation::Replay { continuation, .. } => continuation.to_owned(),
			Continuation::PlayerSeek { .. } => return Err(Error::EndOfContinuation)
		};
		let signaler_topic = match &continuation_contents.live_chat_continuation.continuations[0] {
			Continuation::Invalidation { invalidation_id, .. } => Some(invalidation_id.topic.to_owned()),
			_ => None
		};
		Ok(Self {
			actions: if ctx.live_status.updates_live() {
				continuation_contents
					.live_chat_continuation
					.actions
					.unwrap_or_default()
					.into_iter()
					.map(|f| f.action)
					.collect()
			} else {
				continuation_contents
					.live_chat_continuation
					.actions
					.ok_or(Error::EndOfContinuation)?
					.into_iter()
					.flat_map(|f| match f.action {
						Action::ReplayChat { actions, .. } => actions.into_iter().map(|f| f.action).collect(),
						f => vec![f]
					})
					.collect()
			},
			ctx,
			continuation_token: Some(continuation_token),
			signaler_topic
		})
	}

	pub fn iter(&self) -> std::slice::Iter<'_, Action> {
		self.actions.iter()
	}

	pub async fn cont(&self) -> Option<Result<Self, Error>> {
		if let Some(continuation_token) = &self.continuation_token {
			let page = match GetLiveChatResponse::fetch(self.ctx, continuation_token).await {
				Err(e) => return Some(Err(e)),
				Ok(page) => page
			};
			if page.continuation_contents.is_some() { Some(ActionChunk::new(page, self.ctx)) } else { None }
		} else {
			None
		}
	}
}

impl<'r> IntoIterator for ActionChunk<'r> {
	type Item = Action;
	type IntoIter = std::vec::IntoIter<Action>;

	fn into_iter(self) -> Self::IntoIter {
		self.actions.into_iter()
	}
}

pub async fn stream(options: &ChatContext) -> Result<BoxStream<'_, Result<Action, Error>>, Error> {
	let initial_chat = GetLiveChatResponse::fetch(options, &options.initial_continuation).await?;

	Ok(Box::pin(try_async_stream(|r#yield| async move {
		let mut seen_messages = HashSet::new();

		match &initial_chat.continuation_contents.as_ref().unwrap().live_chat_continuation.continuations[0] {
			Continuation::Invalidation { invalidation_id, .. } => {
				let topic = invalidation_id.topic.to_owned();

				let mut chunk = ActionChunk::new(initial_chat, options)?;

				let mut channel = SignalerChannelInner::with_topic(topic, options.tango_api_key.as_ref().unwrap());
				channel.choose_server().await?;
				channel.init_session().await?;

				for action in chunk.iter() {
					match action {
						Action::AddChatItem { item, .. } => {
							if !seen_messages.contains(item.id()) {
								r#yield(action.to_owned()).await;
								seen_messages.insert(item.id().to_owned());
							}
						}
						Action::ReplayChat { actions, .. } => {
							for action in actions {
								if let Action::AddChatItem { .. } = action.action {
									r#yield(action.action.to_owned()).await;
								}
							}
						}
						action => {
							r#yield(action.to_owned()).await;
						}
					}
				}

				'i: loop {
					match chunk.cont().await {
						Some(Ok(c)) => chunk = c,
						Some(Err(err)) => eprintln!("{err:?}"),
						_ => break 'i
					};

					for action in chunk.iter() {
						match action {
							Action::AddChatItem { item, .. } => {
								if !seen_messages.contains(item.id()) {
									r#yield(action.to_owned()).await;
									seen_messages.insert(item.id().to_owned());
								}
							}
							Action::ReplayChat { actions, .. } => {
								for action in actions {
									if let Action::AddChatItem { .. } = action.action {
										r#yield(action.action.to_owned()).await;
									}
								}
							}
							action => {
								r#yield(action.to_owned()).await;
							}
						}
					}

					let mut req = {
						channel.reset();
						channel.choose_server().await?;
						channel.init_session().await?;
						channel.get_session_stream().await?
					};
					loop {
						match req.chunk().await {
							Ok(Some(s)) => {
								let mut ofs_res_line = s.lines().nth(1).unwrap().unwrap();
								if let Ok(s) = unsafe { simd_json::from_str::<simd_json::OwnedValue>(ofs_res_line.as_mut()) } {
									let a = s.as_array().unwrap();
									{
										channel.aid = a[a.len() - 1].as_array().unwrap()[0].as_usize().unwrap();
									}
								}

								match chunk.cont().await {
									Some(Ok(c)) => chunk = c,
									Some(Err(err)) => eprintln!("{err:?}"),
									_ => break 'i
								};
								channel.topic = chunk.signaler_topic.clone().unwrap();

								for action in chunk.iter() {
									match action {
										Action::AddChatItem { item, .. } => {
											if !seen_messages.contains(item.id()) {
												r#yield(action.to_owned()).await;
												seen_messages.insert(item.id().to_owned());
											}
										}
										Action::ReplayChat { actions, .. } => {
											for action in actions {
												if let Action::AddChatItem { .. } = action.action {
													r#yield(action.action.to_owned()).await;
												}
											}
										}
										action => {
											r#yield(action.to_owned()).await;
										}
									}
								}
							}
							Ok(None) => break,
							Err(e) => {
								eprintln!("{e:?}");
								break;
							}
						}
					}

					seen_messages.clear();
				}
			}
			Continuation::Replay { .. } => {
				let mut chunk = ActionChunk::new(initial_chat, options)?;
				loop {
					for action in chunk.iter() {
						match action {
							Action::AddChatItem { .. } => {
								r#yield(action.to_owned()).await;
							}
							Action::ReplayChat { actions, .. } => {
								for action in actions {
									if let Action::AddChatItem { .. } = action.action {
										r#yield(action.action.to_owned()).await;
									}
								}
							}
							action => {
								r#yield(action.to_owned()).await;
							}
						}
					}
					match chunk.cont().await {
						Some(Ok(e)) => chunk = e,
						_ => break
					}
				}
			}
			Continuation::Timed { timeout_ms, .. } => {
				let timeout = Duration::from_millis(*timeout_ms as _);
				let mut chunk = ActionChunk::new(initial_chat, options)?;
				loop {
					for action in chunk.iter() {
						match action {
							Action::AddChatItem { item, .. } => {
								if !seen_messages.contains(item.id()) {
									r#yield(action.to_owned()).await;
									seen_messages.insert(item.id().to_owned());
								}
							}
							Action::ReplayChat { actions, .. } => {
								for action in actions {
									if let Action::AddChatItem { .. } = action.action {
										r#yield(action.action.to_owned()).await;
									}
								}
							}
							action => {
								r#yield(action.to_owned()).await;
							}
						}
					}
					sleep(timeout).await;
					match chunk.cont().await {
						Some(Ok(e)) => chunk = e,
						_ => break
					}
				}
			}
			Continuation::PlayerSeek { .. } => panic!("player seek should not be first continuation")
		}
		Ok(())
	})))
}
