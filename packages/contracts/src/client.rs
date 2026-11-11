//! The Rust caller of any [`Endpoint`].

use std::{fmt::Write, future::Future, time::Duration};

use reqwest::{RequestBuilder, Response, header};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{ApiError, Auth, Empty, Endpoint, Json, Method, Raw, Stream};

/// The server a call goes to and the credentials it may need.
#[derive(Clone, Copy)]
pub struct Remote<'a> {
    pub http: &'a reqwest::Client,
    /// Everything before the Endpoint's path, such as `https://example.com`.
    pub base: &'a str,
    pub bearer: Option<&'a str>,
    /// Sent in place of `bearer` to [`Auth::Ticket`] routes.
    pub read_ticket: Option<&'a str>,
    pub timeout: Option<Duration>,
}

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("{} (HTTP {status})", error.description)]
    Api { status: u16, error: ApiError },
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("the response (HTTP {status}) could not be read: {source}")]
    Decode {
        status: u16,
        source: serde_json::Error,
    },
}

/// A request body the caller sends.
pub trait Body {
    fn attach(self, request: RequestBuilder) -> RequestBuilder;
}

/// A response body the caller reads whole.
pub trait Reply {
    type Output;
    fn read(
        response: Response,
    ) -> impl Future<Output = Result<Self::Output, CallError>> + Send;
}

pub async fn call<E: Endpoint>(
    remote: &Remote<'_>,
    params: &E::Params,
    query: &E::Query,
    body: E::Body,
) -> Result<<E::Response as Reply>::Output, CallError>
where
    E::Body: Body,
    E::Response: Reply,
{
    let request = body.attach(request::<E>(remote, params, query));
    E::Response::read(check(request.send().await?).await?).await
}

/// Starts a download at byte `from`, returning the response to read as it arrives.
pub async fn call_stream<E: Endpoint<Body = Empty, Response = Stream>>(
    remote: &Remote<'_>,
    params: &E::Params,
    query: &E::Query,
    from: u64,
) -> Result<Response, CallError> {
    let mut request = request::<E>(remote, params, query);
    if from > 0 {
        request = request.header(header::RANGE, format!("bytes={from}-"));
    }
    check(request.send().await?).await
}

fn request<E: Endpoint>(
    remote: &Remote<'_>,
    params: &E::Params,
    query: &E::Query,
) -> RequestBuilder {
    let method = match E::METHOD {
        Method::Get => reqwest::Method::GET,
        Method::Post => reqwest::Method::POST,
        Method::Put => reqwest::Method::PUT,
        Method::Patch => reqwest::Method::PATCH,
        Method::Delete => reqwest::Method::DELETE,
    };
    let mut request = remote
        .http
        .request(method, url::<E>(remote.base, params))
        .query(query);
    let token = match E::AUTH {
        Auth::None => None,
        Auth::Ticket => remote.read_ticket,
        _ => remote.bearer,
    };
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(timeout) = remote.timeout {
        request = request.timeout(timeout);
    }
    request
}

/// `base` and the Endpoint's path with each `{name}` (or `{*name}`, which may hold slashes) filled
/// from the matching field of `params`, percent-encoded.
pub(crate) fn url<E: Endpoint>(base: &str, params: &E::Params) -> String {
    let params = serde_json::to_value(params)
        .expect("an Endpoint's params serialize to JSON");
    let mut url = base.trim_end_matches('/').to_owned();
    for segment in E::PATH.split('/').skip(1) {
        url.push('/');
        let Some(name) =
            segment.strip_prefix('{').and_then(|s| s.strip_suffix('}'))
        else {
            url.push_str(segment);
            continue;
        };
        let (name, keep_slashes) = match name.strip_prefix('*') {
            Some(name) => (name, true),
            None => (name, false),
        };
        let value = match &params[name] {
            Value::String(value) => value.clone(),
            Value::Number(value) => value.to_string(),
            other => panic!("{} has no value for `{name}`: {other}", E::PATH),
        };
        for byte in value.bytes() {
            if byte.is_ascii_alphanumeric()
                || b"-._~".contains(&byte)
                || (keep_slashes && byte == b'/')
            {
                url.push(byte as char);
            } else {
                let _ = write!(url, "%{byte:02X}");
            }
        }
    }
    url
}

async fn check(response: Response) -> Result<Response, CallError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let status = status.as_u16();
    let bytes = response.bytes().await?;
    match serde_json::from_slice(&bytes) {
        Ok(error) => Err(CallError::Api { status, error }),
        Err(source) => Err(CallError::Decode { status, source }),
    }
}

impl<T: Serialize> Body for Json<T> {
    fn attach(self, request: RequestBuilder) -> RequestBuilder {
        request.json(&self.0)
    }
}

impl Body for Empty {
    fn attach(self, request: RequestBuilder) -> RequestBuilder {
        request
    }
}

impl Body for Raw {
    fn attach(self, request: RequestBuilder) -> RequestBuilder {
        request
            .header(header::CONTENT_TYPE, self.content_type)
            .body(self.bytes)
    }
}

impl<T: DeserializeOwned> Reply for Json<T> {
    type Output = T;

    async fn read(response: Response) -> Result<T, CallError> {
        let status = response.status().as_u16();
        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes)
            .map_err(|source| CallError::Decode { status, source })
    }
}

impl Reply for Empty {
    type Output = ();

    async fn read(_: Response) -> Result<(), CallError> {
        Ok(())
    }
}

impl Reply for Raw {
    type Output = Raw;

    async fn read(response: Response) -> Result<Raw, CallError> {
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_owned();
        Ok(Raw {
            content_type,
            bytes: response.bytes().await?,
        })
    }
}
