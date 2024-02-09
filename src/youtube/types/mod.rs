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
use simd_json::OwnedValue;

pub mod get_live_chat;
pub mod streams_page;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommandMetadata {
	pub web_command_metadata: OwnedValue
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnlocalizedText {
	pub simple_text: String,
	pub accessibility: Option<Accessibility>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum LocalizedRun {
	Text {
		text: String
	},
	#[serde(rename_all = "camelCase")]
	Emoji {
		emoji: Emoji,
		variant_ids: Option<Vec<String>>
	}
}

impl LocalizedRun {
	pub fn to_chat_string(&self) -> String {
		match self {
			Self::Text { text } => text.to_owned(),
			Self::Emoji { emoji, .. } => {
				if let Some(true) = emoji.is_custom_emoji {
					format!(":{}:", emoji.image.accessibility.as_ref().unwrap().accessibility_data.label)
				} else {
					emoji.image.accessibility.as_ref().unwrap().accessibility_data.label.to_owned()
				}
			}
		}
	}
}

#[derive(Deserialize, Debug, Clone)]
pub struct LocalizedText {
	pub runs: Vec<LocalizedRun>
}

#[derive(Deserialize, Debug, Clone)]
pub struct ImageContainer {
	pub thumbnails: Vec<Thumbnail>,
	pub accessibility: Option<Accessibility>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Accessibility {
	pub accessibility_data: AccessibilityData
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccessibilityData {
	pub label: String
}

#[derive(Deserialize, Debug, Clone)]
pub struct Thumbnail {
	pub url: String,
	pub width: Option<usize>,
	pub height: Option<usize>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Emoji {
	pub emoji_id: String,
	pub shortcuts: Option<Vec<String>>,
	pub search_terms: Option<Vec<String>>,
	pub supports_skin_tone: Option<bool>,
	pub image: ImageContainer,
	pub is_custom_emoji: Option<bool>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Icon {
	pub icon_type: String
}
