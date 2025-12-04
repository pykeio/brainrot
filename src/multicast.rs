// Copyright 2024 pyke.io
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{pin::Pin, task::Poll};

use futures_util::Stream;
use pin_project_lite::pin_project;
use thiserror::Error;

use crate::{twitch, youtube};

#[derive(Debug, Error)]
pub enum MulticastError {
	#[error("{0}")]
	TwitchError(irc::error::Error),
	#[error("{0}")]
	YouTubeError(youtube::Error)
}

#[derive(Debug)]
pub enum VariantChat {
	Twitch(twitch::ChatEvent),
	YouTube(youtube::ChatEvent)
}

pin_project! {
	#[project = VariantStreamProject]
	enum VariantStream {
		Twitch { #[pin] x: crate::twitch::Chat },
		YouTube { #[pin] x: crate::youtube::Chat }
	}
}

impl Stream for VariantStream {
	type Item = Result<VariantChat, MulticastError>;

	fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
		match self.project() {
			VariantStreamProject::YouTube { x } => {
				Poll::Ready(futures_util::ready!(x.poll_next(cx)).map(|x| x.map(VariantChat::YouTube).map_err(MulticastError::YouTubeError)))
			}
			VariantStreamProject::Twitch { x } => {
				Poll::Ready(futures_util::ready!(x.poll_next(cx)).map(|x| x.map(VariantChat::Twitch).map_err(MulticastError::TwitchError)))
			}
		}
	}
}

impl From<twitch::Chat> for VariantStream {
	fn from(value: twitch::Chat) -> Self {
		Self::Twitch { x: value }
	}
}

impl From<crate::youtube::Chat> for VariantStream {
	fn from(value: youtube::Chat) -> Self {
		Self::YouTube { x: value }
	}
}

pin_project! {
	pub struct Multicast {
		#[pin]
		streams: Vec<VariantStream>
	}
}

impl Multicast {
	pub fn new() -> Self {
		Self { streams: vec![] }
	}

	pub fn push(&mut self, stream: impl Into<VariantStream>) {
		self.streams.push(stream.into());
	}

	pub async fn push_twitch(&mut self, channel: &str, auth: impl twitch::TwitchIdentity) -> Result<(), irc::error::Error> {
		self.push(twitch::Chat::new(channel, auth).await?);
		Ok(())
	}

	pub async fn push_youtube(&mut self, context: youtube::StreamContext) -> Result<(), youtube::Error> {
		self.push(youtube::Chat::new(context).await?);
		Ok(())
	}
}

impl Stream for Multicast {
	type Item = Result<VariantChat, MulticastError>;

	fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
		let mut this = self.project();
		let mut res = Poll::Ready(None);
		for i in 0..this.streams.len() {
			let stream = unsafe { Pin::new_unchecked(this.streams.as_mut().get_unchecked_mut().get_mut(i).unwrap()) };
			match stream.poll_next(cx) {
				Poll::Ready(Some(item)) => return Poll::Ready(Some(item)),
				Poll::Ready(None) => continue,
				Poll::Pending => res = Poll::Pending
			}
		}
		res
	}
}
