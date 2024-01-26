use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

#[derive(Serialize, Debug)]
pub struct GetLiveChatBody {
	context: GetLiveChatBodyContext,
	continuation: String
}

impl GetLiveChatBody {
	pub fn new(continuation: impl Into<String>, client_version: impl Into<String>, client_name: impl Into<String>) -> Self {
		Self {
			context: GetLiveChatBodyContext {
				client: GetLiveChatBodyContextClient {
					client_version: client_version.into(),
					client_name: client_name.into()
				}
			},
			continuation: continuation.into()
		}
	}
}

#[derive(Serialize, Debug)]
pub struct GetLiveChatBodyContext {
	client: GetLiveChatBodyContextClient
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetLiveChatBodyContextClient {
	client_version: String,
	client_name: String
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetLiveChatResponse {
	pub response_context: Option<simd_json::OwnedValue>,
	pub tracking_params: Option<String>,
	pub continuation_contents: Option<GetLiveChatResponseContinuationContents>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetLiveChatResponseContinuationContents {
	pub live_chat_continuation: LiveChatContinuation
}
#[derive(Deserialize, Debug)]
pub struct LiveChatContinuation {
	pub continuations: Vec<Continuation>,
	pub actions: Option<Vec<Action>>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Continuation {
	pub invalidation_continuation_data: Option<InvalidationContinuationData>,
	pub timed_continuation_data: Option<TimedContinuationData>,
	pub live_chat_replay_continuation_data: Option<LiveChatReplayContinuationData>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LiveChatReplayContinuationData {
	pub time_until_last_message_msec: usize,
	pub continuation: String
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InvalidationContinuationData {
	pub invalidation_id: InvalidationId,
	pub timeout_ms: usize,
	pub continuation: String
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TimedContinuationData {
	pub timeout_ms: usize,
	pub continuation: String,
	pub click_tracking_params: Option<String>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Action {
	pub add_chat_item_action: Option<AddChatItemAction>,
	pub add_live_chat_ticker_item_action: Option<simd_json::OwnedValue>,
	pub replay_chat_item_action: Option<ReplayChatItemAction>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReplayChatItemAction {
	pub actions: Vec<Action>,
	#[serde(deserialize_with = "deserialize_number_from_string")]
	pub video_offset_time_msec: i64
}

// MessageRun
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum MessageRun {
	MessageText {
		text: String
	},
	#[serde(rename_all = "camelCase")]
	MessageEmoji {
		emoji: Emoji,
		variant_ids: Option<Vec<String>>
	}
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Emoji {
	pub emoji_id: String,
	pub shortcuts: Option<Vec<String>>,
	pub search_terms: Option<Vec<String>>,
	pub supports_skin_tone: Option<bool>,
	pub image: Image,
	pub is_custom_emoji: Option<bool>
}

#[derive(Deserialize, Debug, Clone)]
pub struct Image {
	pub thumbnails: Vec<Thumbnail>,
	pub accessibility: Accessibility
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
pub struct AuthorBadge {
	pub live_chat_author_badge_renderer: LiveChatAuthorBadgeRenderer
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LiveChatAuthorBadgeRenderer {
	pub custom_thumbnail: Option<CustomThumbnail>,
	pub icon: Option<Icon>,
	pub tooltip: String,
	pub accessibility: Accessibility
}

#[derive(Deserialize, Debug, Clone)]
pub struct CustomThumbnail {
	pub thumbnails: Vec<Thumbnail>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Icon {
	pub icon_type: String
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MessageRendererBase {
	pub author_name: Option<AuthorName>,
	pub author_photo: AuthorPhoto,
	pub author_badges: Option<Vec<AuthorBadge>>,
	pub context_menu_endpoint: ContextMenuEndpoint,
	pub id: String,
	#[serde(deserialize_with = "deserialize_datetime_utc_from_milliseconds")]
	pub timestamp_usec: DateTime<Utc>,
	pub author_external_channel_id: String,
	pub context_menu_accessibility: Accessibility
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContextMenuEndpoint {
	pub click_tracking_params: Option<String>,
	pub command_metadata: CommandMetadata,
	pub live_chat_item_context_menu_endpoint: LiveChatItemContextMenuEndpoint
}

#[derive(Deserialize, Debug, Clone)]
pub struct LiveChatItemContextMenuEndpoint {
	pub params: String
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommandMetadata {
	pub web_command_metadata: WebCommandMetadata
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WebCommandMetadata {
	pub ignore_navigation: bool
}

#[derive(Deserialize, Debug, Clone)]
pub struct AuthorPhoto {
	pub thumbnails: Vec<Thumbnail>
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorName {
	pub simple_text: String
}

#[derive(Deserialize, Debug)]
pub struct LiveChatTextMessageRenderer {
	#[serde(flatten)]
	pub message_renderer_base: MessageRendererBase,
	pub message: Option<Message>
}

#[derive(Deserialize, Debug)]
pub struct Message {
	pub runs: Vec<MessageRun>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LiveChatPaidMessageRenderer {
	#[serde(flatten)]
	pub live_chat_text_message_renderer: LiveChatTextMessageRenderer,
	pub purchase_amount_text: PurchaseAmountText,
	pub header_background_color: isize,
	pub header_text_color: isize,
	pub body_background_color: isize,
	pub body_text_color: isize,
	pub author_name_text_color: isize
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LiveChatPaidStickerRenderer {
	#[serde(flatten)]
	pub message_renderer_base: MessageRendererBase,
	pub purchase_amount_text: PurchaseAmountText,
	pub sticker: Sticker,
	pub money_chip_background_color: isize,
	pub money_chip_text_color: isize,
	pub sticker_display_width: isize,
	pub sticker_display_height: isize,
	pub background_color: isize,
	pub author_name_text_color: isize
}

#[derive(Deserialize, Debug)]
pub struct Sticker {
	pub thumbnails: Vec<Thumbnail>,
	pub accessibility: Accessibility
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseAmountText {
	pub simple_text: String
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LiveChatMembershipItemRenderer {
	#[serde(flatten)]
	pub message_renderer_base: MessageRendererBase,
	pub header_sub_text: Option<HeaderSubText>,
	pub author_badges: Option<Vec<AuthorBadge>>
}

#[derive(Deserialize, Debug)]
pub struct HeaderSubText {
	pub runs: Vec<MessageRun>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddChatItemAction {
	pub item: ActionItem,
	pub client_id: Option<String>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ActionItem {
	pub live_chat_text_message_renderer: Option<LiveChatTextMessageRenderer>,
	pub live_chat_paid_message_renderer: Option<LiveChatPaidMessageRenderer>,
	pub live_chat_membership_item_renderer: Option<LiveChatMembershipItemRenderer>,
	pub live_chat_paid_sticker_renderer: Option<LiveChatPaidStickerRenderer>,
	pub live_chat_viewer_engagement_message_renderer: Option<simd_json::OwnedValue>
}
