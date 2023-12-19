use std::env::args;

use brainrot::ChatEvent;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let mut client = brainrot::Chat::new(args().nth(1).as_deref().unwrap_or("miyukiwei"), brainrot::Anonymous).await?;

	while let Some(message) = client.next().await.transpose()? {
		if let Some(ChatEvent::Message { user, contents, .. }) = brainrot::chat_event(message) {
			println!("{}: {}", user.display_name, contents.iter().map(|c| c.to_string()).collect::<String>());
		}
	}

	Ok(())
}
