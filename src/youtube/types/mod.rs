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

use serde::{de::Error, Deserialize, Deserializer};
use serde_aux::field_attributes::deserialize_number_from_string;
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

pub fn deserialize_datetime_utc_from_microseconds<'de, D>(deserializer: D) -> Result<chrono::DateTime<chrono::Utc>, D::Error>
where
	D: Deserializer<'de>
{
	use chrono::prelude::*;

	let number = deserialize_number_from_string::<i64, D>(deserializer)?;
	let seconds = number / 1_000_000;
	let micros = (number % 1_000_000) as u32;
	let nanos = micros * 1_000;

	Ok(Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(seconds, nanos).ok_or_else(|| D::Error::custom("Couldn't parse the timestamp"))?))
}
