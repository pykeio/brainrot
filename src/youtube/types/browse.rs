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

use serde::{Deserialize, Serialize};

use super::{LocalizedText, deserialize_number_from_string};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseRequest<'s> {
	pub browse_id: &'s str,
	pub params: Option<&'s str>
}

#[derive(Debug, Deserialize)]
pub struct BrowseResponse<'s> {
	#[serde(borrow)]
	pub contents: BrowseResponseContents<'s>
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowseResponseContents<'s> {
	TwoColumnBrowseResultsRenderer {
		#[serde(borrow)]
		tabs: Vec<TabItemRenderer<'s>>
	}
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TabItemRenderer<'s> {
	TabRenderer {
		#[serde(default)]
		selected: bool,
		#[serde(borrow)]
		content: Option<FeedContentsRenderer<'s>>
	},
	#[serde(untagged)]
	#[expect(unused)]
	Other(#[serde(borrow)] simd_json::BorrowedValue<'s>)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedContentsRenderer<'s> {
	RichGridRenderer {
		#[serde(borrow)]
		contents: Vec<RichGridItem<'s>>
	},
	#[serde(untagged)]
	#[expect(unused)]
	Other(#[serde(borrow)] simd_json::BorrowedValue<'s>)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RichGridItem<'s> {
	#[serde(rename_all = "camelCase")]
	RichItemRenderer {
		#[serde(borrow)]
		content: RichItemContent<'s>
	},
	#[serde(untagged)]
	#[expect(unused)]
	Other(#[serde(borrow)] simd_json::BorrowedValue<'s>)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RichItemContent<'s> {
	#[serde(rename_all = "camelCase")]
	VideoRenderer {
		#[serde(borrow)]
		title: LocalizedText<'s>,
		// #[serde(borrow)]
		// thumbnail: ImageContainer<'s>,
		#[serde(borrow)]
		thumbnail_overlays: Vec<ThumbnailOverlay<'s>>,
		video_id: &'s str,
		upcoming_event_data: Option<UpcomingEventData>
	}
}

#[derive(Debug, Deserialize)]
pub struct UpcomingEventData {
	#[serde(rename = "startTime")]
	#[serde(deserialize_with = "deserialize_number_from_string")]
	pub start_time_secs: u64
}

#[derive(Debug, Deserialize)]
pub enum ThumbnailOverlay<'s> {
	#[serde(rename = "thumbnailOverlayTimeStatusRenderer")]
	TimeStatus { style: VideoTimeStatus },
	#[serde(untagged)]
	#[expect(unused)]
	Other(#[serde(borrow)] simd_json::BorrowedValue<'s>)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VideoTimeStatus {
	Upcoming,
	Live,
	Default
}
