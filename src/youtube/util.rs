use std::future::Future;

use reqwest::{RequestBuilder, Response};
use serde::{de::DeserializeOwned, Serialize};

use super::YouTubeError;

pub trait SimdJsonResponseBody {
	fn simd_json<T: DeserializeOwned>(self) -> impl Future<Output = Result<T, YouTubeError>>;
}

impl SimdJsonResponseBody for Response {
	async fn simd_json<T: DeserializeOwned>(self) -> Result<T, YouTubeError> {
		let mut full = self.bytes().await?.to_vec();
		Ok(simd_json::from_slice(&mut full)?)
	}
}

pub trait SimdJsonRequestBody {
	fn simd_json<T: Serialize + ?Sized>(self, json: &T) -> Result<Self, YouTubeError>
	where
		Self: Sized;
}

impl SimdJsonRequestBody for RequestBuilder {
	fn simd_json<T: Serialize + ?Sized>(self, json: &T) -> Result<Self, YouTubeError> {
		Ok(self.body(simd_json::to_vec(json)?))
	}
}
