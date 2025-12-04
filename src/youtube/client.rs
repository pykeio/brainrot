use std::{error::Error as StdError, fmt, future::Future, sync::OnceLock};

use bytes::{Bytes, BytesMut};
use http::{HeaderMap, HeaderName, HeaderValue, Method, Request, Uri, header, request::Builder as RequestBuilder, uri::PathAndQuery};

use crate::youtube::types::{
	self, InnertubeRequest, InnertubeRequestContext, InnertubeRequestContextClient, browse::BrowseRequest, get_live_chat::GetLiveChatRequest,
	video::VideoRequest
};

pub(crate) const DEFAULT_CLIENT_NAME: &str = "WEB";
pub(crate) const DEFAULT_CLIENT_VERSION: &str = "2.20250925.01.00";

pub trait Response: Send + Sized {
	type Error: StdError + Send;

	fn status_code(&self) -> u16;

	fn recv_chunk(&mut self) -> impl Future<Output = Result<Option<Bytes>, Self::Error>> + Send + Sync + '_;

	fn recv_all(mut self) -> impl Future<Output = Result<BytesMut, Self::Error>> + Send {
		async move {
			let mut out = BytesMut::new();
			while let Some(frame) = self.recv_chunk().await? {
				out.extend_from_slice(&frame);
			}
			Ok(out)
		}
	}
}

pub(crate) trait ResponseExt: Response + Sized {
	fn with_innertube_error(self) -> impl Future<Output = Result<Self, InnertubeError>>;
}

impl<T: Response> ResponseExt for T {
	async fn with_innertube_error(self) -> Result<Self, InnertubeError> {
		match self.status_code() {
			200 => Ok(self),
			status_code => {
				let idk = Err(InnertubeError::Unknown { status_code });

				let Ok(mut x) = self.recv_all().await else {
					return idk;
				};

				let Ok(error): Result<types::InnertubeError, _> = simd_json::from_slice(&mut x) else {
					return idk;
				};

				Err(InnertubeError::Specific {
					status_code,
					message: error.message.to_string(),
					code: error.status.to_string()
				})
			}
		}
	}
}

#[derive(Debug)]
pub enum InnertubeError {
	Specific { status_code: u16, message: String, code: String },
	Unknown { status_code: u16 }
}

impl InnertubeError {
	#[inline]
	pub const fn status_code(&self) -> u16 {
		match self {
			Self::Specific { status_code, .. } => *status_code,
			Self::Unknown { status_code } => *status_code
		}
	}
}

impl fmt::Display for InnertubeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Specific { status_code, message, code } => f.write_fmt(format_args!("innertube returned status {status_code}: {message} ({code})")),
			Self::Unknown { status_code } => f.write_fmt(format_args!("innertube returned status {status_code} (couldn't decode body)"))
		}
	}
}

impl StdError for InnertubeError {}

pub trait RequestExecutor: Send + Sync + 'static {
	type Response: Response + 'static;
	type Error: StdError + Send;

	fn make_request(&self, req: http::Request<Bytes>) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send + Sync + '_;
}

#[derive(Debug)]
pub enum ClientError<E> {
	BadRequest(http::Error),
	Serialize(simd_json::Error),
	Executor(E)
}

impl<E> From<simd_json::Error> for ClientError<E> {
	fn from(e: simd_json::Error) -> Self {
		Self::Serialize(e)
	}
}
impl<E> From<http::Error> for ClientError<E> {
	fn from(e: http::Error) -> Self {
		Self::BadRequest(e)
	}
}

impl<E: fmt::Display> fmt::Display for ClientError<E> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Serialize(e) => f.write_fmt(format_args!("body serialization failed: {e}")),
			Self::BadRequest(e) => f.write_fmt(format_args!("accidentally built malformed request: {e}")),
			Self::Executor(e) => f.write_fmt(format_args!("failed to execute request: {e}"))
		}
	}
}

impl<E: StdError> StdError for ClientError<E> {
	fn cause(&self) -> Option<&dyn StdError> {
		match self {
			Self::Serialize(e) => Some(e),
			Self::BadRequest(e) => Some(e),
			Self::Executor(e) => Some(e)
		}
	}
}

#[derive(Debug, Clone)]
pub struct Client<E> {
	http_client: E,
	default_headers: HeaderMap,
	innertube_client: InnertubeRequestContextClient<'static>
}

macro_rules! endpoint {
	($name:ident($body:ty), $path:expr) => {
		#[inline]
		pub(crate) async fn $name(&self, body: $body) -> Result<E::Response, ClientError<E::Error>> {
			static ENDPOINT: OnceLock<Uri> = OnceLock::new();

			#[cold]
			fn url_factory() -> Uri {
				Uri::builder()
					.scheme("https")
					.authority("www.youtube.com")
					.path_and_query(PathAndQuery::from_static(concat!("/youtubei/v1", $path, "?prettyPrint=false")))
					.build()
					.expect("invalid endpoint URI")
			}

			let body = simd_json::to_vec(&InnertubeRequest::new(self, body))?;
			let request = self
				.base_request(ENDPOINT.get_or_init(url_factory).clone())
				.method(Method::POST)
				.header(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))
				.body(body.into())?;
			self.execute(request).await
		}
	};
}

impl<E: RequestExecutor> Client<E> {
	pub fn new(executor: E) -> Self {
		Self::new_with_context(
			executor,
			InnertubeRequestContextClient {
				client_name: DEFAULT_CLIENT_NAME,
				client_version: DEFAULT_CLIENT_VERSION
			}
		)
	}

	pub fn new_with_context(executor: E, context: InnertubeRequestContextClient<'static>) -> Self {
		let mut headers = HeaderMap::new();
		headers.append(
			HeaderName::from_static("x-youtube-client-name"),
			match context.client_name {
				"WEB" => HeaderValue::from_static("1"),
				x => unimplemented!("Unknown client name '{x}'")
			}
		);
		headers.append(HeaderName::from_static("x-youtube-client-version"), HeaderValue::from_str(context.client_version).expect("Invalid client version"));

		// Set our Accept-Language to en-US so we can properly match substrings
		headers.append(header::ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
		headers.append(header::USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:138.0) Gecko/20100101 Firefox/138.0"));
		// Referer is required by Signaler endpoints.
		headers.append(header::REFERER, HeaderValue::from_static("https://www.youtube.com/"));

		Self {
			http_client: executor,
			default_headers: headers,
			innertube_client: context
		}
	}

	#[inline]
	pub(crate) fn request_context(&self) -> InnertubeRequestContext<'_> {
		InnertubeRequestContext {
			client: self.innertube_client.clone()
		}
	}

	pub(crate) fn base_request(&self, uri: Uri) -> RequestBuilder {
		let mut request = Request::builder().uri(uri);
		for (name, value) in self.default_headers.iter() {
			request = request.header(name, value);
		}
		request
	}

	pub(crate) async fn execute(&self, request: http::Request<Bytes>) -> Result<E::Response, ClientError<E::Error>> {
		self.http_client.make_request(request).await.map_err(ClientError::Executor)
	}

	endpoint!(browse(BrowseRequest<'_>), "/browse");
	endpoint!(video(VideoRequest<'_>), "/next");
	endpoint!(chat_live(GetLiveChatRequest<'_>), "/live_chat/get_live_chat");
	endpoint!(chat_replay(GetLiveChatRequest<'_>), "/live_chat/get_live_chat_replay");
}

impl<E: RequestExecutor + Default> Default for Client<E> {
	#[inline]
	fn default() -> Self {
		Client::new(E::default())
	}
}
