use std::{env::args, future::IntoFuture};

use brainrot::youtube;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let (options, cont) = youtube::get_options_from_live_page("6DcXroWNDvk").await?;
	let initial_chat = youtube::fetch_yt_chat_page(&options, cont).await?;
	let subscriber = youtube::SignalerChannel::new_from_cont(&initial_chat).await?;
	let (receiver, handle) = subscriber.spawn_event_subscriber().await?;
	handle.into_future().await.unwrap();
	Ok(())
}
