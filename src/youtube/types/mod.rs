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

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};

use crate::youtube::client::{Client, RequestExecutor};

pub mod browse;
pub mod get_live_chat;
pub mod video;

#[derive(Serialize, Debug)]
pub struct InnertubeRequest<'s, T: Serialize + 's> {
	context: InnertubeRequestContext<'s>,
	#[serde(flatten)]
	body: T
}

impl<'s, T: Serialize + 's> InnertubeRequest<'s, T> {
	pub(crate) fn new<'r: 's, E: RequestExecutor>(client: &'r Client<E>, body: T) -> Self {
		Self {
			context: client.request_context(),
			body
		}
	}
}

#[derive(Serialize, Debug)]
pub struct InnertubeRequestContext<'s> {
	pub client: InnertubeRequestContextClient<'s>
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InnertubeRequestContextClient<'c> {
	pub client_version: &'c str,
	pub client_name: &'c str
}

#[derive(Deserialize, Debug)]
pub struct InnertubeError<'s> {
	pub message: &'s str,
	pub status: &'s str
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnlocalizedText<'s> {
	pub simple_text: &'s str,
	#[serde(borrow)]
	pub accessibility: Option<Accessibility<'s>>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum LocalizedRun<'s> {
	Text {
		text: &'s str
	},
	#[serde(rename_all = "camelCase")]
	Emoji {
		emoji: Emoji<'s>,
		#[serde(borrow)]
		variant_ids: Option<Vec<&'s str>>
	}
}

impl fmt::Display for LocalizedRun<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Text { text } => f.write_str(text),
			Self::Emoji { emoji, .. } => {
				let label = emoji
					.image
					.accessibility
					.as_ref()
					.expect("emojis should always have accessibility data")
					.accessibility_data
					.label;
				if emoji.is_custom_emoji { f.write_fmt(format_args!(":{label}:")) } else { f.write_str(label) }
			}
		}
	}
}

#[derive(Deserialize, Debug, Clone)]
pub struct LocalizedText<'s> {
	#[serde(borrow)]
	pub runs: Vec<LocalizedRun<'s>>
}

#[derive(Deserialize, Debug, Clone)]
pub struct ImageContainer<'s> {
	#[serde(borrow)]
	pub thumbnails: Vec<Thumbnail<'s>>,
	#[serde(borrow)]
	pub accessibility: Option<Accessibility<'s>>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Accessibility<'s> {
	#[serde(borrow)]
	pub accessibility_data: AccessibilityData<'s>
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccessibilityData<'s> {
	pub label: &'s str
}

#[derive(Deserialize, Debug, Clone)]
pub struct Thumbnail<'s> {
	pub url: &'s str,
	pub width: Option<usize>,
	pub height: Option<usize>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Emoji<'s> {
	pub emoji_id: &'s str,
	#[serde(borrow)]
	pub shortcuts: Option<Vec<&'s str>>,
	#[serde(borrow)]
	pub search_terms: Option<Vec<&'s str>>,
	#[serde(default)]
	pub supports_skin_tone: bool,
	pub image: ImageContainer<'s>,
	#[serde(default)]
	pub is_custom_emoji: bool
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Icon<'s> {
	pub icon_type: &'s str
}

pub fn deserialize_number_from_string<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
	D: Deserializer<'de>,
	T: FromStr,
	<T as FromStr>::Err: std::error::Error
{
	let t: &str = Deserialize::deserialize(deserializer)?;
	t.parse().map_err(serde::de::Error::custom)
}
