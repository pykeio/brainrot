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

use std::sync::OnceLock;

use regex::Regex;
use url::Url;

use super::{
	Error, get_http_client,
	types::streams_page::{
		FeedContentsRenderer, PageContentsRenderer, RichGridItem, RichItemContent, TabItemRenderer, ThumbnailOverlay, VideoTimeStatus, YouTubeInitialData
	}
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveStreamStatus {
	Upcoming,
	Live,
	Replay
}

impl LiveStreamStatus {
	#[inline]
	pub fn updates_live(&self) -> bool {
		matches!(self, LiveStreamStatus::Upcoming | LiveStreamStatus::Live)
	}
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ChannelSearchOptions {
	/// Get the live chat of the latest live stream, or the pre-stream chat of the latest upcoming stream if no stream
	/// is currently live.
	LatestLiveOrUpcoming,
	/// Get the live chat of the first live stream, or the pre-stream chat of the first upcoming stream if no stream
	/// is currently live.
	#[default]
	FirstLiveOrUpcoming,
	/// Get the live chat of the first live stream.
	FirstLive,
	/// Get the live chat of the latest live stream.
	LatestLive
}

#[derive(Clone, Debug)]
pub struct ChatContext {
	pub(crate) id: String,
	pub(crate) api_key: String,
	pub(crate) client_version: String,
	pub(crate) initial_continuation: String,
	pub(crate) tango_api_key: Option<String>,
	pub(crate) live_status: LiveStreamStatus
}

impl ChatContext {
	pub async fn new_from_channel(channel_id: impl AsRef<str>, options: ChannelSearchOptions) -> Result<Self, Error> {
		let channel_id = channel_id.as_ref();
		let channel_id = if channel_id.starts_with("UC") || channel_id.starts_with('@') {
			channel_id
		} else {
			Self::parse_channel_link(channel_id).ok_or_else(|| Error::InvalidChannelID(channel_id.to_string()))?
		};
		let page_contents = get_http_client()
			.get(if channel_id.starts_with('@') {
				format!("https://www.youtube.com/{channel_id}/streams")
			} else {
				format!("https://www.youtube.com/channel/{channel_id}/streams")
			})
			.send()
			.await?
			.text()
			.await?;

		static YT_INITIAL_DATA_REGEX: OnceLock<Regex> = OnceLock::new();
		let yt_initial_data: YouTubeInitialData = unsafe {
			simd_json::from_str(
				&mut YT_INITIAL_DATA_REGEX
					.get_or_init(|| Regex::new(r#"var ytInitialData\s*=\s*(\{.+?\});"#).unwrap())
					.captures(&page_contents)
					.ok_or_else(|| Error::NoChatContinuation)?
					.get(1)
					.ok_or(Error::MissingInitialData)?
					.as_str()
					.to_owned()
			)
		}?;

		let mut live_id = None;
		match yt_initial_data.contents {
			PageContentsRenderer::TwoColumnBrowseResultsRenderer { tabs } => match tabs
				.iter()
				.find(|c| match c {
					TabItemRenderer::TabRenderer { title, content, .. } => content.is_some() && title == "Live",
					TabItemRenderer::ExpandableTabRenderer { .. } => false
				})
				.ok_or_else(|| Error::NoMatchingStream(channel_id.to_string()))?
			{
				TabItemRenderer::TabRenderer { content, .. } => match content.as_ref().unwrap() {
					FeedContentsRenderer::RichGridRenderer { contents } => {
						let finder = |c: &&RichGridItem| match c {
							RichGridItem::RichItemRenderer { content, .. } => match content {
								RichItemContent::VideoRenderer { thumbnail_overlays, video_id, .. } => thumbnail_overlays.iter().any(|c| match c {
									ThumbnailOverlay::TimeStatus { style, .. } => {
										if *style == VideoTimeStatus::Live {
											live_id = Some((video_id.to_owned(), true));
											true
										} else {
											if *style == VideoTimeStatus::Upcoming
												&& matches!(options, ChannelSearchOptions::FirstLiveOrUpcoming | ChannelSearchOptions::LatestLiveOrUpcoming)
											{
												match &live_id {
													None => {
														live_id = Some((video_id.to_owned(), false));
													}
													Some((_, false)) => {
														live_id = Some((video_id.to_owned(), false));
													}
													Some((_, true)) => {}
												}
											}
											false
										}
									}
									_ => false
								})
							},
							RichGridItem::ContinuationItemRenderer { .. } => false
						};
						if matches!(options, ChannelSearchOptions::FirstLive | ChannelSearchOptions::FirstLiveOrUpcoming) {
							contents.iter().rev().find(finder)
						} else {
							contents.iter().find(finder)
						}
						.ok_or_else(|| Error::NoMatchingStream(channel_id.to_string()))?
					}
					_ => return Err(Error::NoMatchingStream(channel_id.to_string()))
				},
				TabItemRenderer::ExpandableTabRenderer { .. } => unreachable!()
			}
		};

		ChatContext::new_from_live(live_id.ok_or_else(|| Error::NoMatchingStream(channel_id.to_string()))?.0).await
	}

	pub async fn new_from_live(id: impl AsRef<str>) -> Result<ChatContext, Error> {
		let id = id.as_ref();
		let live_id = if id.is_ascii() && id.len() == 11 {
			id
		} else {
			Self::parse_stream_link(id).ok_or_else(|| Error::InvalidVideoID(id.to_string()))?
		};
		let page_contents = get_http_client()
			.get(format!("https://www.youtube.com/watch?v={live_id}"))
			.send()
			.await?
			.text()
			.await?;

		static LIVE_STREAM_REGEX: OnceLock<Regex> = OnceLock::new();
		let live_status = if LIVE_STREAM_REGEX
			.get_or_init(|| Regex::new(r#"['"]isLiveContent['"]:\s*(true)"#).unwrap())
			.find(&page_contents)
			.is_some()
		{
			static LIVE_NOW_REGEX: OnceLock<Regex> = OnceLock::new();
			static REPLAY_REGEX: OnceLock<Regex> = OnceLock::new();
			if LIVE_NOW_REGEX
				.get_or_init(|| Regex::new(r#"['"]isLiveNow['"]:\s*(true)"#).unwrap())
				.find(&page_contents)
				.is_some()
			{
				LiveStreamStatus::Live
			} else if REPLAY_REGEX
				.get_or_init(|| Regex::new(r#"['"]isReplay['"]:\s*(true)"#).unwrap())
				.find(&page_contents)
				.is_some()
			{
				LiveStreamStatus::Replay
			} else {
				LiveStreamStatus::Upcoming
			}
		} else {
			return Err(Error::NotStream(live_id.to_string()));
		};

		static INNERTUBE_API_KEY_REGEX: OnceLock<Regex> = OnceLock::new();
		let api_key = match INNERTUBE_API_KEY_REGEX
			.get_or_init(|| Regex::new(r#"['"]INNERTUBE_API_KEY['"]:\s*['"](.+?)['"]"#).unwrap())
			.captures(&page_contents)
			.and_then(|captures| captures.get(1))
		{
			Some(matched) => matched.as_str().to_string(),
			None => return Err(Error::NoInnerTubeKey)
		};

		static TANGO_API_KEY_REGEX: OnceLock<Regex> = OnceLock::new();
		let tango_api_key = TANGO_API_KEY_REGEX
			.get_or_init(|| Regex::new(r#"['"]LIVE_CHAT_BASE_TANGO_CONFIG['"]:\s*\{\s*['"]apiKey['"]\s*:\s*['"](.+?)['"]"#).unwrap())
			.captures(&page_contents)
			.and_then(|captures| captures.get(1).map(|c| c.as_str().to_string()));

		static CLIENT_VERSION_REGEX: OnceLock<Regex> = OnceLock::new();
		let client_version = match CLIENT_VERSION_REGEX
			.get_or_init(|| Regex::new(r#"['"]clientVersion['"]:\s*['"]([\d.]+?)['"]"#).unwrap())
			.captures(&page_contents)
			.and_then(|captures| captures.get(1))
		{
			Some(matched) => matched.as_str().to_string(),
			None => "2.20240207.07.00".to_string()
		};

		static LIVE_CONTINUATION_REGEX: OnceLock<Regex> = OnceLock::new();
		static REPLAY_CONTINUATION_REGEX: OnceLock<Regex> = OnceLock::new();
		let continuation_regex = if live_status.updates_live() {
			LIVE_CONTINUATION_REGEX.get_or_init(|| Regex::new(
				r#"Live chat['"],\s*['"]selected['"]:\s*(?:true|false),\s*['"]continuation['"]:\s*\{\s*['"]reloadContinuationData['"]:\s*\{['"]continuation['"]:\s*['"](.+?)['"]"#
			).unwrap())
		} else {
			REPLAY_CONTINUATION_REGEX.get_or_init(|| {
				Regex::new(
					r#"Top chat replay['"],\s*['"]selected['"]:\s*true,\s*['"]continuation['"]:\s*\{\s*['"]reloadContinuationData['"]:\s*\{['"]continuation['"]:\s*['"](.+?)['"]"#
				)
				.unwrap()
			})
		};
		let continuation = match continuation_regex.captures(&page_contents).and_then(|captures| captures.get(1)) {
			Some(matched) => matched.as_str().to_string(),
			None => return Err(Error::NoChatContinuation)
		};

		Ok(ChatContext {
			id: live_id.to_string(),
			api_key,
			client_version,
			tango_api_key,
			initial_continuation: continuation,
			live_status
		})
	}

	fn parse_stream_link(url: &str) -> Option<&str> {
		static LINK_RE: OnceLock<Regex> = OnceLock::new();
		LINK_RE
			.get_or_init(|| Regex::new(r#"(?:https?:\/\/)?(?:www\.)?youtu\.?be(?:\.com)?\/?.*(?:watch|embed)?(?:.*v=|v\/|\/)([A-Za-z0-9-_]+)"#).unwrap())
			.captures(url)
			.and_then(|c| c.get(1))
			.map(|c| c.as_str())
	}

	fn parse_channel_link(url: &str) -> Option<&str> {
		static CHANNEL_RE: OnceLock<Regex> = OnceLock::new();
		CHANNEL_RE
			.get_or_init(|| Regex::new(r#"^(?:https?:\/\/)?(?:www\.)?youtube\.com\/(?:channel\/(UC[\w-]{21}[AQgw])|(@[\w]+))$"#).unwrap())
			.captures(url)
			.and_then(|c| c.get(1).or_else(|| c.get(2)))
			.map(|c| c.as_str())
	}

	pub fn id(&self) -> &str {
		&self.id
	}

	pub fn url(&self) -> Url {
		Url::parse(&format!("https://www.youtube.com/watch?v={}", self.id)).unwrap()
	}

	pub fn status(&self) -> LiveStreamStatus {
		self.live_status
	}
}
