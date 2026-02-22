use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRequest<'s> {
	pub video_id: &'s str
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VideoResponse<'s> {
	#[serde(borrow)]
	pub contents: VideoResponseContents<'s>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum VideoResponseContents<'s> {
	#[serde(rename_all = "camelCase")]
	TwoColumnWatchNextResults {
		#[serde(borrow)]
		conversation_bar: Option<ConversationBar<'s>>
	}
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ConversationBar<'s> {
	#[serde(rename_all = "camelCase")]
	LiveChatRenderer {
		#[serde(borrow)]
		continuations: Vec<ContinuationData<'s>>,
		#[serde(default)]
		is_replay: bool
	},
	#[serde(untagged)]
	#[allow(unused)]
	Other(#[serde(borrow)] simd_json::BorrowedValue<'s>)
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ContinuationData<'s> {
	#[serde(rename_all = "camelCase")]
	ReloadContinuationData { continuation: &'s str }
}
