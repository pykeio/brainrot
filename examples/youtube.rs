use std::env::args;

use brainrot::youtube;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let (options, cont) = youtube::get_options_from_live_page("5Z5Sys8-tLs").await?;
	let initial_chat = youtube::fetch_yt_chat_page(&options, cont).await?;
	youtube::subscribe_to_events(&options, &initial_chat).await?;
	Ok(())
}
