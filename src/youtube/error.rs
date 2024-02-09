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

use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
	#[error("Invalid YouTube video ID or URL: {0}")]
	InvalidVideoID(String),
	#[error("Invalid YouTube channel ID or URL: {0}")]
	InvalidChannelID(String),
	#[error("Channel {0} has no live stream matching the options criteria")]
	NoMatchingStream(String),
	#[error("Missing `ytInitialData` structure from channel streams page.")]
	MissingInitialData,
	#[error("error when deserializing: {0}")]
	Deserialization(#[from] simd_json::Error),
	#[error("missing continuation contents")]
	MissingContinuationContents,
	#[error("reached end of continuation")]
	EndOfContinuation,
	#[error("request timed out")]
	TimedOut,
	#[error("request returned bad HTTP status: {0}")]
	BadStatus(StatusCode),
	#[error("request error: {0}")]
	GeneralRequest(reqwest::Error),
	#[error("{0} is not a live stream")]
	NotStream(String),
	#[error("Failed to match InnerTube API key")]
	NoInnerTubeKey,
	#[error("Chat continuation token could not be found.")]
	NoChatContinuation,
	#[error("Error parsing URL: {0}")]
	URLParseError(#[from] url::ParseError)
}

impl Error {
	pub fn is_fatal(&self) -> bool {
		!matches!(self, Error::TimedOut)
	}
}

impl From<reqwest::Error> for Error {
	fn from(value: reqwest::Error) -> Self {
		if value.is_timeout() {
			Error::TimedOut
		} else if value.is_status() {
			Error::BadStatus(value.status().unwrap())
		} else {
			Error::GeneralRequest(value)
		}
	}
}
