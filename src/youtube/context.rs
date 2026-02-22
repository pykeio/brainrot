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

use std::{error::Error as StdError, fmt};

use crate::youtube::{
	ClientError,
	client::{Client, InnertubeError, RequestExecutor, Response, ResponseExt},
	types::video::{ContinuationData, ConversationBar, VideoRequest, VideoResponse, VideoResponseContents}
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StreamChatMode {
	/// Top chat, filtering out potential spam
	#[default]
	Top,
	/// Live chat with minimal filtering
	Live
}

#[derive(Debug)]
pub struct StreamContext<E: RequestExecutor> {
	pub(crate) client: Client<E>,
	pub(crate) initial_continuation: String,
	pub(crate) is_replay: bool
}

impl<E: RequestExecutor> StreamContext<E> {
	pub async fn new(client: Client<E>, id: impl AsRef<str>, mode: StreamChatMode) -> Result<Self, StreamContextError<E>> {
		let id = id.as_ref();
		if !id.is_ascii() || id.len() != 11 {
			return Err(StreamContextError::InvalidVideoID);
		}

		let mut video_response = client
			.video(VideoRequest { video_id: id })
			.await?
			.with_innertube_error()
			.await?
			.recv_all()
			.await
			.map_err(StreamContextError::Receive)?;
		let video_response: VideoResponse<'_> = simd_json::from_slice(&mut video_response)?;

		let VideoResponseContents::TwoColumnWatchNextResults {
			conversation_bar: Some(conversation_bar)
		} = video_response.contents
		else {
			return Err(StreamContextError::NoChat);
		};

		let ConversationBar::LiveChatRenderer { continuations, is_replay } = conversation_bar else {
			return Err(StreamContextError::NoChat);
		};

		let Some(ContinuationData::ReloadContinuationData { continuation }) = continuations.first() else {
			return Err(StreamContextError::NoChat);
		};

		let mut continuation = continuation.to_string();

		if mode == StreamChatMode::Live {
			// All continuation tokens are base64url-encoded protobuf. The byte sequence `08 08 xx 18` is present in all
			// of them - `xx` determines whether the top chat or live chat is used, where top chat is `01` and live chat is `04`.
			// YT API only provides 01 (top chat) tokens. In lieu of manually building the protobuf messages ourselves
			// and pulling in a base64 encoder/decoder, have this extremely fragile and extremely stupid mechanism instead.
			if is_replay {
				if let Some((index, pat)) = continuation.match_indices("NEQAFyCAgEGAIgAC").next() {
					continuation.replace_range(index..index + pat.len(), "NEQAFyCAgBGAIgAC");
				} else {
					tracing::warn!("failed to find sentinel in continuation token; top chat will be used instead");
				}
			} else if let Some((index, pat)) = continuation.match_indices("RDABggEICAQYAiAAKAC").next() {
				continuation.replace_range(index..index + pat.len(), "RDABggEICAEYAiAAKAC");
			} else {
				tracing::warn!("failed to find sentinel in continuation token; top chat will be used instead");
			}
		}

		Ok(StreamContext {
			client,
			initial_continuation: continuation,
			is_replay
		})
	}
}

#[derive(Debug)]
pub enum StreamContextError<E: RequestExecutor> {
	InvalidVideoID,
	NoChat,
	Deserialize(simd_json::Error),
	Client(ClientError<E::Error>),
	Receive(<E::Response as Response>::Error),
	Innertube(InnertubeError)
}

impl<E: RequestExecutor> From<simd_json::Error> for StreamContextError<E> {
	fn from(e: simd_json::Error) -> Self {
		Self::Deserialize(e)
	}
}
impl<E: RequestExecutor> From<ClientError<E::Error>> for StreamContextError<E> {
	fn from(e: ClientError<E::Error>) -> Self {
		Self::Client(e)
	}
}
impl<E: RequestExecutor> From<InnertubeError> for StreamContextError<E> {
	fn from(e: InnertubeError) -> Self {
		Self::Innertube(e)
	}
}

impl<E: RequestExecutor> fmt::Display for StreamContextError<E> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidVideoID => f.write_str("invalid video ID"),
			Self::NoChat => f.write_str("stream does not have chat enabled"),
			Self::Deserialize(e) => f.write_fmt(format_args!("failed to deserialize response: {e}")),
			Self::Client(e) => fmt::Display::fmt(e, f),
			Self::Receive(e) => f.write_fmt(format_args!("failed to receive response: {e}")),
			Self::Innertube(e) => fmt::Display::fmt(e, f)
		}
	}
}

impl<E: RequestExecutor + fmt::Debug> StdError for StreamContextError<E>
where
	E::Response: fmt::Debug
{
	fn cause(&self) -> Option<&dyn StdError> {
		match self {
			Self::Deserialize(e) => Some(e),
			Self::Client(e) => Some(e),
			Self::Receive(e) => Some(e),
			Self::Innertube(e) => Some(e),
			_ => None
		}
	}
}
