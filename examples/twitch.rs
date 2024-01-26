use std::env::args;

use brainrot::{twitch, TwitchChat, TwitchChatEvent};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let mut client = TwitchChat::new(args().nth(1).as_deref().unwrap_or("miyukiwei"), twitch::Anonymous).await?;

	while let Some(message) = client.next().await.transpose()? {
		if let TwitchChatEvent::Message { user, contents, .. } = message {
			println!("{}: {}", user.display_name, contents.iter().map(|c| c.to_string()).collect::<String>());
		}
	}

	Ok(())
}
