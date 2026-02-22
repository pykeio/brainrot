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
	error::Error as StdError,
	fmt::{self, Write}
};

use super::client::{Client, ClientError, InnertubeError, RequestExecutor, Response, ResponseExt};
use crate::youtube::{
	LocalizedRun,
	types::browse::{
		BrowseRequest, BrowseResponse, BrowseResponseContents, FeedContentsRenderer, RichGridItem, RichItemContent, TabItemRenderer, ThumbnailOverlay,
		VideoTimeStatus
	}
};

pub(crate) const TANGO_API_KEY: &str = "AIzaSyDZNkyC-AtROwMBpLfevIvqYk-Gfi8ZOeo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStatus {
	Live,
	Upcoming { scheduled_secs: u64 }
}

#[derive(Debug)]
pub struct ChannelStream {
	video_id: String,
	title: String,
	status: StreamStatus,
	thumbnail_url: Option<String>
}

impl ChannelStream {
	#[inline(always)]
	pub fn id(&self) -> &str {
		&self.video_id
	}

	#[inline(always)]
	pub fn title(&self) -> &str {
		&self.title
	}

	#[inline(always)]
	pub fn status(&self) -> StreamStatus {
		self.status
	}

	#[inline(always)]
	pub fn thumbnail_url(&self) -> Option<&str> {
		self.thumbnail_url.as_deref()
	}
}

pub(crate) fn stringify_runs(runs: &[LocalizedRun<'_>]) -> String {
	let mut s = String::new();
	for run in runs {
		write!(&mut s, "{run}").expect("infallible");
	}
	s
}

pub async fn query_channel<E: RequestExecutor>(channel_id: &str, client: &Client<E>) -> Result<Vec<ChannelStream>, QueryChannelError<E>> {
	if !channel_id.starts_with("UC") || channel_id.len() != 24 {
		return Err(QueryChannelError::InvalidChannelID);
	}

	let mut browse_results = client
		.browse(BrowseRequest {
			browse_id: channel_id,
			// streams tab
			params: Some("EgdzdHJlYW1z8gYECgJ6AA%3D%3D")
		})
		.await?
		.with_innertube_error()
		.await?
		.recv_all()
		.await
		.map_err(QueryChannelError::Receive)?;
	let browse_results: BrowseResponse<'_> = simd_json::from_slice(&mut browse_results)?;

	let BrowseResponseContents::TwoColumnBrowseResultsRenderer { tabs } = browse_results.contents;
	let Some(TabItemRenderer::TabRenderer { content: stream_tab_renderer, .. }) = tabs.iter().find(|c| match c {
		TabItemRenderer::TabRenderer { selected, content, .. } => content.is_some() && *selected,
		_ => false
	}) else {
		tracing::warn!("Failed to find stream tab renderer");
		return Ok(Vec::new());
	};

	let Some(FeedContentsRenderer::RichGridRenderer { contents: stream_items }) = stream_tab_renderer else {
		tracing::warn!("Stream tab wasn't a `richGridRenderer`");
		return Ok(Vec::new());
	};

	Ok(stream_items
		.iter()
		.filter_map(|c| match c {
			RichGridItem::RichItemRenderer { content } => match content {
				RichItemContent::VideoRenderer {
					thumbnail_overlays,
					video_id,
					title,
					upcoming_event_data,
					..
				} => {
					let time_status = thumbnail_overlays.iter().find_map(|c| match c {
						ThumbnailOverlay::TimeStatus { style, .. } => Some(style),
						_ => None
					})?;

					if matches!(time_status, VideoTimeStatus::Default) {
						return None;
					}

					let video_id = video_id.to_string();
					let title = stringify_runs(&title.runs);
					let thumbnail = format!("https://i.ytimg.com/vi/{video_id}/maxresdefault.jpg"); // 1280x720, innertube only gives us 336x118 at most

					match time_status {
						VideoTimeStatus::Live => Some(ChannelStream {
							video_id,
							title,
							thumbnail_url: Some(thumbnail),
							status: StreamStatus::Live
						}),
						VideoTimeStatus::Upcoming => Some(ChannelStream {
							video_id,
							title,
							thumbnail_url: Some(thumbnail),
							status: StreamStatus::Upcoming {
								scheduled_secs: upcoming_event_data
									.as_ref()
									.expect("`upcomingEventData` should be present for UPCOMING streams")
									.start_time_secs
							}
						}),
						_ => unreachable!()
					}
				}
			},
			_ => None
		})
		.collect())
}

#[derive(Debug)]
pub enum QueryChannelError<E: RequestExecutor> {
	InvalidChannelID,
	Deserialize(simd_json::Error),
	Client(ClientError<E::Error>),
	Receive(<E::Response as Response>::Error),
	Innertube(InnertubeError)
}

impl<E: RequestExecutor> From<simd_json::Error> for QueryChannelError<E> {
	fn from(e: simd_json::Error) -> Self {
		Self::Deserialize(e)
	}
}
impl<E: RequestExecutor> From<ClientError<E::Error>> for QueryChannelError<E> {
	fn from(e: ClientError<E::Error>) -> Self {
		Self::Client(e)
	}
}
impl<E: RequestExecutor> From<InnertubeError> for QueryChannelError<E> {
	fn from(e: InnertubeError) -> Self {
		Self::Innertube(e)
	}
}

impl<E: RequestExecutor> fmt::Display for QueryChannelError<E> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidChannelID => f.write_str("invalid channel ID"),
			Self::Deserialize(e) => f.write_fmt(format_args!("failed to deserialize response: {e}")),
			Self::Client(e) => fmt::Display::fmt(e, f),
			Self::Receive(e) => f.write_fmt(format_args!("failed to receive response: {e}")),
			Self::Innertube(e) => fmt::Display::fmt(e, f)
		}
	}
}

impl<E: RequestExecutor + fmt::Debug> StdError for QueryChannelError<E>
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
