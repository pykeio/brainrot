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

pub trait MapNonempty {
	type T;

	fn and_then_nonempty<B, F>(self, f: F) -> Option<B>
	where
		Self: Sized,
		F: FnOnce(Self::T) -> Option<B>;
}

impl MapNonempty for String {
	type T = String;

	fn and_then_nonempty<B, F>(self, f: F) -> Option<B>
	where
		Self: Sized,
		F: FnOnce(Self::T) -> Option<B>
	{
		if self.is_empty() { None } else { f(self) }
	}
}

impl MapNonempty for Option<String> {
	type T = String;

	fn and_then_nonempty<B, F>(self, f: F) -> Option<B>
	where
		Self: Sized,
		F: FnOnce(Self::T) -> Option<B>
	{
		self.and_then(|c| c.and_then_nonempty(f))
	}
}

pub fn get_utf8_slice(s: &str, start: usize, end: usize) -> Option<&str> {
	let mut iter = s.char_indices().map(|(pos, _)| pos).chain(Some(s.len())).skip(start).peekable();
	let start_pos = *iter.peek()?;
	for _ in start..end {
		iter.next();
	}
	Some(&s[start_pos..*iter.peek()?])
}
