//! The Rust caller of any [`Endpoint`].

use std::{fmt::Write, future::Future, time::Duration};

use reqwest::{RequestBuilder, Response, StatusCode, header};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    ApiError, Auth, Empty, Endpoint, Json, Method, Raw, Stream,
    tickets::READ_TICKET_HEADER,
};

/// The server a call goes to and the credentials it may need.
#[derive(Clone, Copy)]
pub struct Remote<'a> {
    pub http: &'a reqwest::Client,
    /// Everything before the Endpoint's path, such as `https://example.com`.
    pub base: &'a str,
    pub bearer: Option<&'a str>,
    /// Sent in place of `bearer` to [`Auth::Ticket`] routes, and beside it to [`Auth::Account`]
    /// ones in [`READ_TICKET_HEADER`].
    pub read_ticket: Option<&'a str>,
    pub timeout: Option<Duration>,
}

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("{} (HTTP {status})", error.description)]
    Api { status: u16, error: ApiError },
    /// Without its URL, which can carry a read ticket in the query.
    #[error(transparent)]
    Http(reqwest::Error),
    #[error("the response (HTTP {status}) could not be read: {source}")]
    Decode {
        status: u16,
        source: serde_json::Error,
    },
    /// A download resumed at a byte past 0 was answered without `206 Partial Content`.
    #[error("the server ignored the requested range (HTTP {status})")]
    RangeIgnored { status: u16 },
    /// A path segment would be empty, `.` or `..`, which would send the call to another route.
    #[error("`{value}` is not a valid `{name}` for {path}")]
    Params {
        path: &'static str,
        name: String,
        value: String,
    },
}

impl From<reqwest::Error> for CallError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error.without_url())
    }
}

/// An [`ApiError`] is a code and a short description; a longer error body is refused unread.
const MAX_ERROR_BYTES: usize = 64 * 1024;

/// A request body the caller sends: the marker itself, or the body a [`Stream`] sends.
pub trait Body {
    type Input;
    fn attach(input: Self::Input, request: RequestBuilder) -> RequestBuilder;
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
    body: <E::Body as Body>::Input,
) -> Result<<E::Response as Reply>::Output, CallError>
where
    E::Body: Body,
    E::Response: Reply,
{
    let request = E::Body::attach(body, request::<E>(remote, params, query)?);
    E::Response::read(check(request.send().await?).await?).await
}

/// [`call`] for a server trusted no further than its answer's first `max` bytes: a longer JSON
/// answer is refused unread.
pub async fn call_limited<E, T>(
    remote: &Remote<'_>,
    params: &E::Params,
    query: &E::Query,
    body: <E::Body as Body>::Input,
    max: usize,
) -> Result<T, CallError>
where
    E: Endpoint<Response = Json<T>>,
    E::Body: Body,
    T: DeserializeOwned,
{
    let request = E::Body::attach(body, request::<E>(remote, params, query)?);
    let response = check(request.send().await?).await?;
    let status = response.status().as_u16();
    let bytes = read_up_to(response, max).await?;
    serde_json::from_slice(&bytes)
        .map_err(|source| CallError::Decode { status, source })
}

/// Starts a download at byte `from`, returning the response to read as it arrives.
pub async fn call_stream<E: Endpoint<Body = Empty, Response = Stream>>(
    remote: &Remote<'_>,
    params: &E::Params,
    query: &E::Query,
    from: u64,
) -> Result<Response, CallError> {
    let mut request = request::<E>(remote, params, query)?;
    if from > 0 {
        request = request.header(header::RANGE, format!("bytes={from}-"));
    }
    let response = check(request.send().await?).await?;
    let status = response.status();
    if from > 0 && status != StatusCode::PARTIAL_CONTENT {
        return Err(CallError::RangeIgnored {
            status: status.as_u16(),
        });
    }
    Ok(response)
}

fn request<E: Endpoint>(
    remote: &Remote<'_>,
    params: &E::Params,
    query: &E::Query,
) -> Result<RequestBuilder, CallError> {
    let method = match E::METHOD {
        Method::Get => reqwest::Method::GET,
        Method::Post => reqwest::Method::POST,
        Method::Put => reqwest::Method::PUT,
        Method::Patch => reqwest::Method::PATCH,
        Method::Delete => reqwest::Method::DELETE,
    };
    let mut request = remote
        .http
        .request(method, url::<E>(remote.base, params)?)
        .query(query);
    let token = match E::AUTH {
        Auth::None => None,
        Auth::Ticket => remote.read_ticket,
        _ => remote.bearer,
    };
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if E::AUTH == Auth::Account
        && let Some(ticket) = remote.read_ticket
    {
        request = request.header(READ_TICKET_HEADER, ticket);
    }
    if let Some(timeout) = remote.timeout {
        request = request.timeout(timeout);
    }
    Ok(request)
}

/// `base` and the Endpoint's path with each `{name}` (or `{*name}`, which may hold slashes) filled
/// from the matching field of `params`, percent-encoded.
pub fn url<E: Endpoint>(
    base: &str,
    params: &E::Params,
) -> Result<String, CallError> {
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
        // URL parsers resolve dot segments, even percent-encoded ones.
        let dot = |piece: &str| matches!(piece, "" | "." | "..");
        let invalid = if keep_slashes {
            value.split('/').any(dot)
        } else {
            dot(&value)
        };
        if invalid {
            return Err(CallError::Params {
                path: E::PATH,
                name: name.to_owned(),
                value,
            });
        }
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
    Ok(url)
}

async fn check(response: Response) -> Result<Response, CallError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let bytes = read_up_to(response, MAX_ERROR_BYTES).await?;
    match serde_json::from_slice(&bytes) {
        Ok(error) => Err(CallError::Api { status, error }),
        Err(source) => Err(CallError::Decode { status, source }),
    }
}

async fn read_up_to(
    mut response: Response,
    max: usize,
) -> Result<Vec<u8>, CallError> {
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > max {
            return Err(CallError::Decode {
                status,
                source: serde::de::Error::custom("the body is too long"),
            });
        }
    }
    Ok(bytes)
}

impl<T: Serialize> Body for Json<T> {
    type Input = Self;

    fn attach(input: Self, request: RequestBuilder) -> RequestBuilder {
        request.json(&input.0)
    }
}

impl Body for Empty {
    type Input = Self;

    fn attach(_: Self, request: RequestBuilder) -> RequestBuilder {
        request
    }
}

impl Body for Raw {
    type Input = Self;

    fn attach(input: Self, request: RequestBuilder) -> RequestBuilder {
        request
            .header(header::CONTENT_TYPE, input.content_type)
            .body(input.bytes)
    }
}

impl Body for Stream {
    type Input = reqwest::Body;

    fn attach(input: reqwest::Body, request: RequestBuilder) -> RequestBuilder {
        request.body(input)
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

#[cfg(test)]
mod tests {
    use reqwest::Client;
    use serde::{Deserialize, Serialize};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::{
        CallError, MAX_ERROR_BYTES, Remote, call, call_limited, call_stream,
        url,
    };
    use crate::{Auth, Empty, Endpoint, ErrorCode, Json, Method, Stream};

    #[derive(Serialize, Deserialize)]
    struct File {
        id: String,
        path: String,
    }

    struct ReadFile;

    impl Endpoint for ReadFile {
        const METHOD: Method = Method::Get;
        const PATH: &'static str = "/v1/instances/{id}/files/{*path}";
        const AUTH: Auth = Auth::Session;
        type Params = File;
        type Query = ();
        type Body = Empty;
        type Response = Json<String>;
    }

    fn file(id: &str, path: &str) -> File {
        File {
            id: id.into(),
            path: path.into(),
        }
    }

    #[test]
    fn a_param_cannot_leave_its_route() {
        assert_eq!(
            url::<ReadFile>("", &file("a", "config/x.json")).unwrap(),
            "/v1/instances/a/files/config/x.json"
        );
        for (id, path) in
            [("..", "x"), ("", "x"), ("a", "config/../.."), ("a", "./x")]
        {
            assert!(matches!(
                url::<ReadFile>("", &file(id, path)),
                Err(CallError::Params { .. })
            ));
        }
    }

    struct Download;

    impl Endpoint for Download {
        const METHOD: Method = Method::Get;
        const PATH: &'static str = "/v1/download";
        const AUTH: Auth = Auth::None;
        type Params = ();
        type Query = ();
        type Body = Empty;
        type Response = Stream;
    }

    /// A server that answers one request with `response`.
    struct Server {
        http: Client,
        base: String,
    }

    impl Server {
        async fn answer(response: String) -> Self {
            // Another crate in the workspace build turns on reqwest's `rustls-no-provider`.
            let _ = rustls::crypto::ring::default_provider().install_default();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let _ = socket.read(&mut [0; 4096]).await;
                // The client may stop reading early.
                let _ = socket.write_all(response.as_bytes()).await;
            });
            Self {
                http: Client::new(),
                base,
            }
        }

        fn remote(&self) -> Remote<'_> {
            Remote {
                http: &self.http,
                base: &self.base,
                bearer: None,
                read_ticket: None,
                timeout: None,
            }
        }
    }

    async fn read_file(response: impl Into<String>) -> CallError {
        let server = Server::answer(response.into()).await;
        call::<ReadFile>(&server.remote(), &file("a", "b"), &(), Empty)
            .await
            .unwrap_err()
    }

    async fn resume(response: &str) -> Result<(), CallError> {
        let server = Server::answer(response.into()).await;
        call_stream::<Download>(&server.remote(), &(), &(), 10)
            .await
            .map(drop)
    }

    #[derive(Serialize, Deserialize)]
    struct Ticket {
        read_ticket: String,
    }

    struct ReadWithTicket;

    impl Endpoint for ReadWithTicket {
        const METHOD: Method = Method::Get;
        const PATH: &'static str = "/v1/read";
        const AUTH: Auth = Auth::None;
        type Params = ();
        type Query = Ticket;
        type Body = Empty;
        type Response = Json<String>;
    }

    struct ReadAsAccount;

    impl Endpoint for ReadAsAccount {
        const METHOD: Method = Method::Get;
        const PATH: &'static str = "/v1/read";
        const AUTH: Auth = Auth::Account;
        type Params = ();
        type Query = ();
        type Body = Empty;
        type Response = Json<String>;
    }

    #[tokio::test]
    async fn a_read_ticket_goes_to_an_account_route_in_its_header() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let received = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut head = [0; 4096];
            let read = socket.read(&mut head).await.unwrap();
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n\"\"")
                .await;
            String::from_utf8_lossy(&head[..read]).to_lowercase()
        });
        let http = Client::new();
        let remote = Remote {
            http: &http,
            base: &base,
            bearer: Some("amc_key"),
            read_ticket: Some("secret"),
            timeout: None,
        };
        call::<ReadAsAccount>(&remote, &(), &(), Empty)
            .await
            .unwrap();
        let request = received.await.unwrap();
        assert!(request.starts_with("get /v1/read http/1.1"), "{request}");
        assert!(
            request.contains("authorization: bearer amc_key"),
            "{request}"
        );
        assert!(
            request.contains("x-amberite-read-ticket: secret"),
            "{request}"
        );
    }

    #[tokio::test]
    async fn a_transport_error_leaves_the_query_out() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = Client::new();
        let remote = Remote {
            http: &http,
            base: &base,
            bearer: None,
            read_ticket: None,
            timeout: None,
        };
        let query = Ticket {
            read_ticket: "secret".into(),
        };
        let error = call::<ReadWithTicket>(&remote, &(), &query, Empty)
            .await
            .unwrap_err();
        assert!(matches!(error, CallError::Http(_)));
        assert!(!format!("{error} {error:?}").contains("secret"));
    }

    #[tokio::test]
    async fn a_resumed_download_needs_the_range_it_asked_for() {
        let error = resume(
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nwhole",
        )
        .await
        .unwrap_err();
        assert!(matches!(error, CallError::RangeIgnored { status: 200 }));

        resume(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 10-13/14\r\n\
             Content-Length: 4\r\nConnection: close\r\n\r\nrest",
        )
        .await
        .unwrap();
    }

    async fn read_limited(max: usize) -> Result<String, CallError> {
        let server = Server::answer(
            "HTTP/1.1 200 OK
Content-Length: 7
Connection: close

\"short\""
                .into(),
        )
        .await;
        call_limited::<ReadFile, _>(
            &server.remote(),
            &file("a", "b"),
            &(),
            Empty,
            max,
        )
        .await
    }

    #[tokio::test]
    async fn bodies_are_read_only_up_to_their_cap() {
        assert_eq!(read_limited(7).await.unwrap(), "short");
        assert!(matches!(
            read_limited(6).await.unwrap_err(),
            CallError::Decode { status: 200, .. }
        ));

        let body = format!(
            "{{\"error\":\"not_found\",\"description\":\"Gone.\"}}{}",
            " ".repeat(MAX_ERROR_BYTES)
        );
        let error = read_file(format!(
            "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ))
        .await;
        assert!(matches!(error, CallError::Decode { status: 404, .. }));
    }

    #[tokio::test]
    async fn the_client_reads_error_bodies() {
        let error = read_file(
            "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: 49\r\n\
             Connection: close\r\n\r\n{\"error\":\"from_the_future\",\"description\":\"Gone.\"}",
        )
        .await;
        assert!(matches!(
            error,
            CallError::Api { status: 404, error } if error.error == ErrorCode::Unknown
        ));

        let error = read_file(
            "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Gateway",
        )
        .await;
        assert!(matches!(error, CallError::Decode { status: 502, .. }));
    }
}
