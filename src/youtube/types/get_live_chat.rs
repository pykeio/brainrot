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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;
use url::Url;

use super::{deserialize_datetime_utc_from_microseconds, Accessibility, CommandMetadata, Icon, ImageContainer, LocalizedText, UnlocalizedText};
use crate::youtube::{
	get_http_client,
	util::{SimdJsonRequestBody, SimdJsonResponseBody},
	ChatContext, Error, TANGO_LIVE_ENDPOINT, TANGO_REPLAY_ENDPOINT
};

#[derive(Serialize, Debug)]
pub struct GetLiveChatRequestBody {
	context: GetLiveChatRequestBodyContext,
	continuation: String
}

impl GetLiveChatRequestBody {
	pub(crate) fn new(continuation: impl Into<String>, client_version: impl Into<String>, client_name: impl Into<String>) -> Self {
		Self {
			context: GetLiveChatRequestBodyContext {
				client: GetLiveChatRequestBodyContextClient {
					client_version: client_version.into(),
					client_name: client_name.into()
				}
			},
			continuation: continuation.into()
		}
	}
}

#[derive(Serialize, Debug)]
pub struct GetLiveChatRequestBodyContext {
	client: GetLiveChatRequestBodyContextClient
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetLiveChatRequestBodyContextClient {
	client_version: String,
	client_name: String
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetLiveChatResponse {
	pub response_context: Option<simd_json::OwnedValue>,
	pub continuation_contents: Option<GetLiveChatResponseContinuationContents>
}

impl GetLiveChatResponse {
	pub async fn fetch(options: &ChatContext, continuation: impl AsRef<str>) -> Result<Self, Error> {
		let body = GetLiveChatRequestBody::new(continuation.as_ref(), &options.client_version, "WEB");
		Ok(get_http_client()
			.post(Url::parse_with_params(
				if options.live_status.updates_live() { TANGO_LIVE_ENDPOINT } else { TANGO_REPLAY_ENDPOINT },
				[("key", options.api_key.as_str()), ("prettyPrint", "false")]
			)?)
			.simd_json(&body)?
			.send()
			.await?
			.simd_json()
			.await
			.unwrap())
	}
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetLiveChatResponseContinuationContents {
	pub live_chat_continuation: LiveChatContinuation
}

#[derive(Deserialize, Debug)]
pub struct LiveChatContinuation {
	pub continuations: Vec<Continuation>,
	pub actions: Option<Vec<ActionContainer>>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ActionContainer {
	#[serde(flatten)]
	pub action: Action,
	pub click_tracking_params: Option<String>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Continuation {
	#[serde(rename = "invalidationContinuationData")]
	#[serde(rename_all = "camelCase")]
	Invalidation {
		invalidation_id: InvalidationId,
		timeout_ms: usize,
		continuation: String
	},
	#[serde(rename = "timedContinuationData")]
	#[serde(rename_all = "camelCase")]
	Timed { timeout_ms: usize, continuation: String },
	#[serde(rename = "liveChatReplayContinuationData")]
	#[serde(rename_all = "camelCase")]
	Replay { time_until_last_message_msec: usize, continuation: String },
	#[serde(rename = "playerSeekContinuationData")]
	#[serde(rename_all = "camelCase")]
	PlayerSeek { continuation: String }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InvalidationId {
	pub object_source: usize,
	pub object_id: String,
	pub topic: String,
	pub subscribe_to_gcm_topics: bool,
	pub proto_creation_timestamp_ms: String
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Action {
	#[serde(rename = "addChatItemAction")]
	#[serde(rename_all = "camelCase")]
	AddChatItem {
		item: ChatItem,
		client_id: Option<String>
	},
	#[serde(rename = "removeChatItemAction")]
	#[serde(rename_all = "camelCase")]
	RemoveChatItem {
		target_item_id: String
	},
	#[serde(rename = "removeChatItemByAuthorAction")]
	#[serde(rename_all = "camelCase")]
	RemoveChatItemByAuthor {
		external_channel_id: String
	},
	#[serde(rename = "addLiveChatTickerItemAction")]
	#[serde(rename_all = "camelCase")]
	AddLiveChatTicker {
		item: simd_json::OwnedValue
	},
	#[serde(rename = "replayChatItemAction")]
	#[serde(rename_all = "camelCase")]
	ReplayChat {
		actions: Vec<ActionContainer>,
		#[serde(deserialize_with = "deserialize_number_from_string")]
		video_offset_time_msec: i64
	},
	LiveChatReportModerationStateCommand(simd_json::OwnedValue)
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorBadge {
	pub live_chat_author_badge_renderer: LiveChatAuthorBadgeRenderer
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LiveChatAuthorBadgeRenderer {
	pub custom_thumbnail: Option<ImageContainer>,
	pub icon: Option<Icon>,
	pub tooltip: String,
	pub accessibility: Accessibility
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MessageRendererBase {
	pub author_name: Option<UnlocalizedText>,
	pub author_photo: ImageContainer,
	pub author_badges: Option<Vec<AuthorBadge>>,
	pub context_menu_endpoint: ContextMenuEndpoint,
	pub id: String,
	#[serde(deserialize_with = "deserialize_datetime_utc_from_microseconds")]
	pub timestamp_usec: DateTime<Utc>,
	pub author_external_channel_id: String,
	pub context_menu_accessibility: Accessibility
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContextMenuEndpoint {
	pub command_metadata: CommandMetadata,
	pub live_chat_item_context_menu_endpoint: LiveChatItemContextMenuEndpoint
}

#[derive(Deserialize, Debug, Clone)]
pub struct LiveChatItemContextMenuEndpoint {
	pub params: String
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ChatItem {
	#[serde(rename = "liveChatTextMessageRenderer")]
	#[serde(rename_all = "camelCase")]
	TextMessage {
		#[serde(flatten)]
		message_renderer_base: MessageRendererBase,
		message: Option<LocalizedText>
	},
	#[serde(rename = "liveChatPaidMessageRenderer")]
	#[serde(rename_all = "camelCase")]
	Superchat {
		#[serde(flatten)]
		message_renderer_base: MessageRendererBase,
		message: Option<LocalizedText>,
		purchase_amount_text: UnlocalizedText,
		header_background_color: isize,
		header_text_color: isize,
		body_background_color: isize,
		body_text_color: isize,
		author_name_text_color: isize
	},
	#[serde(rename = "liveChatMembershipItemRenderer")]
	#[serde(rename_all = "camelCase")]
	MembershipItem {
		#[serde(flatten)]
		message_renderer_base: MessageRendererBase,
		header_sub_text: Option<LocalizedText>,
		author_badges: Option<Vec<AuthorBadge>>
	},
	#[serde(rename = "liveChatPaidStickerRenderer")]
	#[serde(rename_all = "camelCase")]
	PaidSticker {
		#[serde(flatten)]
		message_renderer_base: MessageRendererBase,
		purchase_amount_text: UnlocalizedText,
		sticker: ImageContainer,
		money_chip_background_color: isize,
		money_chip_text_color: isize,
		sticker_display_width: isize,
		sticker_display_height: isize,
		background_color: isize,
		author_name_text_color: isize
	},
	#[serde(rename = "liveChatSponsorshipsGiftPurchaseAnnouncementRenderer")]
	#[serde(rename_all = "camelCase")]
	MembershipGift {
		id: String,
		#[serde(flatten)]
		data: simd_json::OwnedValue
	},
	#[serde(rename = "liveChatSponsorshipsGiftRedemptionAnnouncementRenderer")]
	#[serde(rename_all = "camelCase")]
	MembershipGiftRedemption {
		id: String,
		#[serde(flatten)]
		data: simd_json::OwnedValue
	},
	#[serde(rename = "liveChatViewerEngagementMessageRenderer")]
	ViewerEngagement { id: String }
}

impl ChatItem {
	pub fn id(&self) -> &str {
		match self {
			ChatItem::MembershipItem { message_renderer_base, .. } => &message_renderer_base.id,
			ChatItem::PaidSticker { message_renderer_base, .. } => &message_renderer_base.id,
			ChatItem::Superchat { message_renderer_base, .. } => &message_renderer_base.id,
			ChatItem::TextMessage { message_renderer_base, .. } => &message_renderer_base.id,
			ChatItem::MembershipGift { id, .. } => id,
			ChatItem::MembershipGiftRedemption { id, .. } => id,
			ChatItem::ViewerEngagement { id } => id
		}
	}
}
