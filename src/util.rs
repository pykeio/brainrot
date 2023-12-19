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
