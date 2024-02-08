use std::{future::IntoFuture, time::Duration};

use brainrot::youtube::{self, YouTubeChatPageProcessor};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let (options, cont) = youtube::get_options_from_live_page("J2YmJL0PX5M").await?;
	let initial_chat = youtube::fetch_yt_chat_page(&options, &cont).await?;
	if let Some(invalidation_continuation) = initial_chat.continuation_contents.as_ref().unwrap().live_chat_continuation.continuations[0]
		.invalidation_continuation_data
		.as_ref()
	{
		let topic = invalidation_continuation.invalidation_id.topic.to_owned();
		let subscriber = youtube::SignalerChannel::new(topic).await?;
		let (mut receiver, _handle) = subscriber.spawn_event_subscriber().await?;
		tokio::spawn(async move {
			let mut processor = YouTubeChatPageProcessor::new(initial_chat, &options).unwrap();
			for msg in &processor {
				println!("{}: {}", msg.author.display_name, msg.runs.iter().map(|c| c.to_string()).collect::<String>());
			}

			while receiver.recv().await.is_ok() {
				match processor.cont().await {
					Some(Ok(s)) => {
						processor = s;
						for msg in &processor {
							println!("{}: {}", msg.author.display_name, msg.runs.iter().map(|c| c.to_string()).collect::<String>());
						}

						subscriber.refresh_topic(processor.signaler_topic.as_ref().unwrap()).await;
					}
					Some(Err(e)) => {
						eprintln!("{e:?}");
						break;
					}
					None => {
						eprintln!("none");
						break;
					}
				}
			}
		});
		_handle.into_future().await.unwrap();
	} else if let Some(timed_continuation) = initial_chat.continuation_contents.as_ref().unwrap().live_chat_continuation.continuations[0]
		.timed_continuation_data
		.as_ref()
	{
		let timeout = timed_continuation.timeout_ms as u64;
		let mut processor = YouTubeChatPageProcessor::new(initial_chat, &options).unwrap();
		loop {
			for msg in &processor {
				println!("{}: {}", msg.author.display_name, msg.runs.iter().map(|c| c.to_string()).collect::<String>());
			}
			sleep(Duration::from_millis(timeout as _)).await;
			match processor.cont().await {
				Some(Ok(e)) => processor = e,
				_ => break
			}
		}
	}
	println!("???");
	Ok(())
}
