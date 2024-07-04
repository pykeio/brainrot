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

use serde::Deserialize;

use super::{Accessibility, CommandMetadata, ImageContainer, LocalizedText};

#[derive(Debug, Deserialize)]
pub struct YouTubeInitialData {
	pub contents: PageContentsRenderer
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageContentsRenderer {
	TwoColumnBrowseResultsRenderer { tabs: Vec<TabItemRenderer> }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TabItemRenderer {
	TabRenderer {
		endpoint: FeedEndpoint,
		title: String,
		#[serde(default)]
		selected: bool,
		content: Option<FeedContentsRenderer>
	},
	ExpandableTabRenderer {}
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedEndpoint {
	pub browse_endpoint: BrowseEndpoint,
	pub command_metadata: CommandMetadata
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpoint {
	pub browse_id: String,
	pub params: String,
	pub canonical_base_url: String
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedContentsRenderer {
	RichGridRenderer {
		contents: Vec<RichGridItem>
	},
	#[serde(other)]
	Other
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RichGridItem {
	#[serde(rename_all = "camelCase")]
	RichItemRenderer { content: RichItemContent },
	#[serde(rename_all = "camelCase")]
	ContinuationItemRenderer { trigger: ContinuationItemTrigger }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RichItemContent {
	#[serde(rename_all = "camelCase")]
	VideoRenderer {
		description_snippet: LocalizedText,
		thumbnail: ImageContainer,
		thumbnail_overlays: Vec<ThumbnailOverlay>,
		video_id: String
	}
}

#[derive(Debug, Deserialize)]
pub enum ThumbnailOverlay {
	#[serde(rename = "thumbnailOverlayTimeStatusRenderer")]
	TimeStatus {
		style: VideoTimeStatus // text: UnlocalizedText
	},
	#[serde(rename = "thumbnailOverlayToggleButtonRenderer")]
	#[serde(rename_all = "camelCase")]
	ToggleButton {
		is_toggled: Option<bool>,
		toggled_accessibility: Accessibility,
		toggled_tooltip: String,
		untoggled_accessibility: Accessibility,
		untoggled_tooltip: String
	},
	#[serde(rename = "thumbnailOverlayNowPlayingRenderer")]
	NowPlaying { text: LocalizedText }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VideoTimeStatus {
	Upcoming,
	Live,
	Default
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContinuationItemTrigger {
	ContinuationTriggerOnItemShown
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedHeaderRenderer {
	#[serde(rename_all = "camelCase")]
	FeedFilterChipBarRenderer { contents: Vec<FeedFilterChip>, style_type: String }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedFilterChip {
	#[serde(rename_all = "camelCase")]
	ChipCloudChipRenderer { is_selected: bool }
}
