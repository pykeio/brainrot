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

use std::future::Future;

use reqwest::{RequestBuilder, Response};
use serde::{de::DeserializeOwned, Serialize};

use super::Error;

pub trait SimdJsonResponseBody {
	fn simd_json<T: DeserializeOwned>(self) -> impl Future<Output = Result<T, Error>>;
}

impl SimdJsonResponseBody for Response {
	async fn simd_json<T: DeserializeOwned>(self) -> Result<T, Error> {
		let mut full = self.bytes().await?.to_vec();
		Ok(simd_json::from_slice(&mut full)?)
	}
}

pub trait SimdJsonRequestBody {
	fn simd_json<T: Serialize + ?Sized>(self, json: &T) -> Result<Self, Error>
	where
		Self: Sized;
}

impl SimdJsonRequestBody for RequestBuilder {
	fn simd_json<T: Serialize + ?Sized>(self, json: &T) -> Result<Self, Error> {
		Ok(self.body(simd_json::to_vec(json)?))
	}
}
