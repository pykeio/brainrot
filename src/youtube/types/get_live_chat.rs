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

use super::{Accessibility, Icon, ImageContainer, LocalizedText, UnlocalizedText, deserialize_number_from_string};

#[derive(Serialize, Debug)]
pub struct GetLiveChatRequest<'s> {
	pub continuation: &'s str
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetLiveChatResponse<'s> {
	#[serde(bound = "GetLiveChatResponseContinuationContents<'s>: serde::Deserialize<'de>")]
	pub continuation_contents: Option<GetLiveChatResponseContinuationContents<'s>>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetLiveChatResponseContinuationContents<'s> {
	#[serde(bound = "LiveChatContinuation<'s>: serde::Deserialize<'de>")]
	pub live_chat_continuation: LiveChatContinuation<'s>
}

#[derive(Deserialize, Debug)]
pub struct LiveChatContinuation<'s> {
	#[serde(borrow)]
	pub continuations: Vec<Continuation<'s>>,
	#[serde(default)]
	#[serde(bound = "Vec<ActionContainer<'s>>: serde::Deserialize<'de>")]
	pub actions: Vec<ActionContainer<'s>>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ActionContainer<'s> {
	#[serde(rename = "clickTrackingParams")]
	_tracking: Option<&'s str>,
	#[serde(flatten)]
	#[serde(bound = "simd_json::BorrowedValue<'s>: serde::Deserialize<'de>")]
	pub action: simd_json::BorrowedValue<'s>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Continuation<'s> {
	#[serde(rename = "invalidationContinuationData")]
	#[serde(rename_all = "camelCase")]
	Invalidation {
		#[serde(borrow)]
		invalidation_id: InvalidationId<'s>,
		// timeout_ms: usize,
		continuation: &'s str
	},
	#[serde(rename = "timedContinuationData")]
	#[serde(rename_all = "camelCase")]
	Timed { timeout_ms: usize, continuation: &'s str },
	#[serde(rename = "liveChatReplayContinuationData")]
	#[serde(rename_all = "camelCase")]
	Replay { continuation: &'s str },
	#[serde(rename = "playerSeekContinuationData")]
	#[serde(rename_all = "camelCase")]
	#[allow(unused)]
	PlayerSeek { continuation: &'s str }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InvalidationId<'s> {
	pub topic: &'s str
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Action<'s> {
	#[serde(rename = "addChatItemAction")]
	#[serde(rename_all = "camelCase")]
	AddChatItem {
		#[serde(borrow)]
		item: ChatItem<'s>,
		client_id: Option<&'s str>
	},
	#[serde(rename = "addLiveChatTickerItemAction")]
	#[serde(rename_all = "camelCase")]
	AddTickerItem {
		#[serde(flatten)]
		#[serde(bound(deserialize = "simd_json::BorrowedValue<'s>: serde::Deserialize<'de>"))]
		data: simd_json::BorrowedValue<'s>
	},
	#[serde(rename = "replaceChatItemAction")]
	#[serde(rename_all = "camelCase")]
	ReplaceChatItem {
		target_item_id: &'s str,
		#[serde(borrow)]
		replacement_item: ChatItem<'s>
	},
	#[serde(rename = "removeChatItemAction")]
	#[serde(rename_all = "camelCase")]
	RemoveChatItem { target_item_id: &'s str },
	#[serde(rename = "removeChatItemByAuthorAction")]
	#[serde(rename_all = "camelCase")]
	RemoveChatItemByAuthor { external_channel_id: &'s str },
	#[serde(rename = "replayChatItemAction")]
	#[serde(rename_all = "camelCase")]
	ReplayChat {
		#[serde(borrow)]
		actions: Vec<ActionContainer<'s>>,
		#[serde(deserialize_with = "deserialize_number_from_string")]
		video_offset_time_msec: i64
	},
	#[serde(rename = "addBannerToLiveChatCommand")]
	#[serde(rename_all = "camelCase")]
	AddBannerToLiveChat {
		#[serde(flatten)]
		#[serde(bound(deserialize = "simd_json::BorrowedValue<'s>: serde::Deserialize<'de>"))]
		data: simd_json::BorrowedValue<'s>
	},
	#[serde(rename = "liveChatReportModerationStateCommand")]
	ReportModerationState {
		#[serde(flatten)]
		#[serde(bound(deserialize = "simd_json::BorrowedValue<'s>: serde::Deserialize<'de>"))]
		data: simd_json::BorrowedValue<'s>
	}
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorBadge<'s> {
	#[serde(borrow)]
	pub live_chat_author_badge_renderer: LiveChatAuthorBadgeRenderer<'s>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LiveChatAuthorBadgeRenderer<'s> {
	#[serde(borrow)]
	pub custom_thumbnail: Option<ImageContainer<'s>>,
	#[serde(borrow)]
	pub icon: Option<Icon<'s>>,
	pub tooltip: &'s str,
	#[serde(borrow)]
	pub accessibility: Accessibility<'s>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MessageRendererBase<'s> {
	pub id: &'s str,
	#[serde(borrow)]
	pub author_name: Option<UnlocalizedText<'s>>,
	#[serde(borrow)]
	pub author_photo: ImageContainer<'s>,
	#[serde(borrow, default)]
	pub author_badges: Vec<AuthorBadge<'s>>,
	#[serde(deserialize_with = "deserialize_number_from_string")]
	pub timestamp_usec: i64,
	pub author_external_channel_id: &'s str
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ChatItem<'s> {
	#[serde(rename = "liveChatTextMessageRenderer")]
	#[serde(rename_all = "camelCase")]
	TextMessage {
		#[serde(borrow, flatten)]
		base: MessageRendererBase<'s>,
		#[serde(borrow)]
		message: Option<LocalizedText<'s>>
	},
	#[serde(rename = "liveChatPaidMessageRenderer")]
	#[serde(rename_all = "camelCase")]
	Superchat {
		#[serde(borrow, flatten)]
		base: MessageRendererBase<'s>,
		#[serde(borrow)]
		message: Option<LocalizedText<'s>>,
		#[serde(borrow)]
		purchase_amount_text: UnlocalizedText<'s>,
		header_background_color: isize,
		header_text_color: isize,
		body_background_color: isize,
		body_text_color: isize,
		author_name_text_color: isize
	},
	#[serde(rename = "liveChatMembershipItemRenderer")]
	#[serde(rename_all = "camelCase")]
	MembershipItem {
		#[serde(borrow, flatten)]
		base: MessageRendererBase<'s>,
		#[serde(borrow)]
		header_sub_text: Option<LocalizedText<'s>>
	},
	#[serde(rename = "liveChatPaidStickerRenderer")]
	#[serde(rename_all = "camelCase")]
	PaidSticker {
		#[serde(borrow, flatten)]
		base: MessageRendererBase<'s>,
		#[serde(borrow)]
		purchase_amount_text: UnlocalizedText<'s>,
		#[serde(borrow)]
		sticker: ImageContainer<'s>,
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
		id: &'s str,
		#[serde(deserialize_with = "deserialize_number_from_string")]
		timestamp_usec: i64,
		author_external_channel_id: &'s str,
		#[serde(borrow)]
		header: ChatItemHeader<'s>
	},
	#[serde(rename = "liveChatSponsorshipsGiftRedemptionAnnouncementRenderer")]
	#[serde(rename_all = "camelCase")]
	MembershipGiftRedemption {
		#[serde(borrow, flatten)]
		base: MessageRendererBase<'s>,
		#[serde(borrow)]
		message: Option<LocalizedText<'s>>
	},
	#[serde(rename = "liveChatPlaceholderItemRenderer")]
	#[serde(rename_all = "camelCase")]
	Placeholder {
		id: &'s str,
		#[serde(deserialize_with = "deserialize_number_from_string")]
		timestamp_usec: i64
	},
	#[serde(rename = "liveChatViewerEngagementMessageRenderer")]
	ViewerEngagement { id: &'s str },
	#[serde(untagged)]
	Unknown(#[serde(bound(deserialize = "simd_json::BorrowedValue<'s>: serde::Deserialize<'de>"))] simd_json::BorrowedValue<'s>)
}

impl ChatItem<'_> {
	pub fn id(&self) -> &str {
		match self {
			ChatItem::MembershipItem { base, .. } => base.id,
			ChatItem::PaidSticker { base, .. } => base.id,
			ChatItem::Superchat { base, .. } => base.id,
			ChatItem::TextMessage { base, .. } => base.id,
			ChatItem::MembershipGift { id, .. } => id,
			ChatItem::MembershipGiftRedemption { base, .. } => base.id,
			ChatItem::Placeholder { id, .. } => id,
			ChatItem::ViewerEngagement { id } => id,
			ChatItem::Unknown(_) => ""
		}
	}
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ChatItemHeader<'s> {
	#[serde(rename = "liveChatSponsorshipsHeaderRenderer")]
	#[serde(rename_all = "camelCase")]
	Sponsorship {
		#[serde(borrow)]
		author_name: Option<UnlocalizedText<'s>>,
		#[serde(borrow)]
		author_photo: ImageContainer<'s>,
		#[serde(borrow, default)]
		author_badges: Vec<AuthorBadge<'s>>,
		#[serde(borrow)]
		primary_text: LocalizedText<'s>
	}
}
