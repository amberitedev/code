//! The local address the WebView uses for one Core: an HTTP/1.1 proxy on 127.0.0.1 that carries
//! each request to the Core over pinned TLS and signs it with this device's session there.
//!
//! The WebView's own `Authorization` is dropped, so the account session can never reach a Core,
//! and the session token is added here, so the WebView never holds it. Console sockets are
//! upgraded through the proxy too: a browser `WebSocket` cannot set headers at all.
//!
//! Only the App may use the address. Its first path segment is a capability: 32 random bytes in
//! hex, made for each proxy and given only to the WebView, in the base URL `core_connect`
//! returns. A request without it, from a process on this computer that found the port or anything
//! else, gets 404 and nothing is signed. This matters because a Core often runs on the same PC,
//! where a mod runs as another OS user. The capability is stripped before the request goes on.
//!
//! The `Origin` must also be the App's. Every request from the WebView carries one
//! (`tauri-plugin-http` adds it, browsers always send it on XHR and WebSockets), so a request from
//! any other origin, or with none, is refused before it is signed. That includes a web page that
//! learned the address, and one that rebinds a DNS name to this computer.

use std::{convert::Infallible, sync::Arc, sync::LazyLock};

use bytes::Bytes;
use http_body_util::{Either, Full};
use hyper::{
    Method, Request, Response, StatusCode, Uri,
    body::Incoming,
    client::conn::http1::SendRequest,
    header::{
        ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
        ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_MAX_AGE,
        ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD,
        AUTHORIZATION, CONTENT_TYPE, COOKIE, HeaderValue, ORIGIN, UPGRADE,
    },
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use rand::{RngCore, rngs::OsRng};
use tokio::{io::copy_bidirectional, net::TcpListener, sync::Mutex};

use super::{
    sessions::{Key, Sessions},
    tls::Target,
};

/// Set to print the head of every request the proxy sends to a Core, with the session token left
/// out, and whether the WebView had sent an `Authorization` of its own.
const LOG_VAR: &str = "AMBERITE_CORE_PROXY_LOG";
static LOG: LazyLock<bool> =
    LazyLock::new(|| std::env::var_os(LOG_VAR).is_some());

/// One Core, and whose session to sign its requests with.
pub struct Proxy {
    pub target: Target,
    pub sessions: Arc<Sessions>,
    pub key: Key,
    /// The first path segment every request must start with. See [`capability`].
    pub capability: String,
}

/// A new capability: 32 random bytes from the OS, in hex.
pub fn capability() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

type Reply = Response<Either<Incoming, Full<Bytes>>>;
/// The connection to the Core behind one local connection, opened on its first request.
type Upstream = Arc<Mutex<Option<SendRequest<Incoming>>>>;

/// Serve `listener` for as long as the App runs.
pub async fn serve(listener: TcpListener, proxy: Arc<Proxy>) {
    while let Ok((stream, _)) = listener.accept().await {
        let proxy = Arc::clone(&proxy);
        tokio::spawn(async move {
            let upstream = Upstream::default();
            let service = service_fn(move |request| {
                handle(Arc::clone(&proxy), Arc::clone(&upstream), request)
            });
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .with_upgrades()
                .await;
        });
    }
}

async fn handle(
    proxy: Arc<Proxy>,
    upstream: Upstream,
    mut request: Request<Incoming>,
) -> Result<Reply, Infallible> {
    let Some(uri) = without_capability(request.uri(), &proxy.capability) else {
        return Ok(error(StatusCode::NOT_FOUND, "not_found", "Not found."));
    };
    *request.uri_mut() = uri;
    let Some(origin) = request
        .headers()
        .get(ORIGIN)
        .filter(|origin| allowed_origin(origin))
        .cloned()
    else {
        return Ok(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Only the Amberite App can use this address.",
        ));
    };
    // A session is only ever made through `core_sign_in`, which keeps its token out of the
    // WebView. Here the answer would hand the WebView a raw token.
    if request.method() == Method::POST && request.uri().path() == "/sessions" {
        return Ok(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Sign in to a Core through the App.",
        ));
    }
    if request.method() == Method::OPTIONS
        && request
            .headers()
            .contains_key(ACCESS_CONTROL_REQUEST_METHOD)
    {
        let mut response = preflight(&request);
        response
            .headers_mut()
            .insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        return Ok(response);
    }

    let headers = request.headers_mut();
    let webview_authorization = headers.remove(AUTHORIZATION).is_some();
    // Core refuses any request with an `Origin`: only a browser sends one. Cookies are the
    // browser's for every port on this computer, and Core uses none.
    headers.remove(ORIGIN);
    headers.remove(COOKIE);
    if let Some(token) = proxy.sessions.token(&proxy.key).await
        && let Ok(value) = HeaderValue::try_from(format!("Bearer {token}"))
    {
        headers.insert(AUTHORIZATION, value);
    }
    if *LOG {
        log(&request, webview_authorization);
    }

    let sent = if request.headers().contains_key(UPGRADE) {
        upgrade(&proxy, request).await
    } else {
        forward(&proxy, &upstream, request).await
    };
    let mut response = sent.unwrap_or_else(|problem| {
        tracing::debug!(%problem, "a Core did not answer");
        error(
            StatusCode::BAD_GATEWAY,
            "unreachable",
            "The Core did not answer.",
        )
    });
    response
        .headers_mut()
        .insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    Ok(response)
}

/// The URI with its first path segment taken off, when that segment is `capability`. Compared in
/// constant time, so the answer's timing says nothing about how much of a guess was right.
fn without_capability(uri: &Uri, capability: &str) -> Option<Uri> {
    let rest = uri.path().strip_prefix('/')?;
    let (segment, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (given, expected) = (segment.as_bytes(), capability.as_bytes());
    let differs = given
        .iter()
        .zip(expected)
        .fold(given.len() ^ expected.len(), |differs, (a, b)| {
            differs | usize::from(a ^ b)
        });
    if differs != 0 {
        return None;
    }
    let path_and_query = match uri.query() {
        Some(query) => format!("/{path}?{query}"),
        None => format!("/{path}"),
    };
    Uri::try_from(path_and_query).ok()
}

/// The App's own origins. Development builds also serve it from loopback: the Vite dev server and
/// the browser bridge.
fn allowed_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    if matches!(
        origin,
        "tauri://localhost"
            | "http://tauri.localhost"
            | "https://tauri.localhost"
    ) {
        return true;
    }
    if !(cfg!(debug_assertions) || cfg!(feature = "browser-bridge")) {
        return false;
    }
    let Ok(url) = url::Url::parse(origin) else {
        return false;
    };
    url.scheme() == "http"
        && match url.host() {
            Some(url::Host::Domain(name)) => name == "localhost",
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            None => false,
        }
}

/// Answer a CORS preflight for the WebView's own uploads, which go through XHR.
fn preflight(request: &Request<Incoming>) -> Reply {
    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(
            ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, PUT, PATCH, DELETE",
        )
        .header(ACCESS_CONTROL_MAX_AGE, "600");
    if let Some(requested) =
        request.headers().get(ACCESS_CONTROL_REQUEST_HEADERS)
    {
        response = response.header(ACCESS_CONTROL_ALLOW_HEADERS, requested);
    }
    response
        .body(Either::Right(Full::default()))
        .expect("a valid response")
}

/// Send a request on this local connection's connection to the Core, opening it first if there is
/// none or it closed. A request the closed connection never sent goes on a new one.
async fn forward(
    proxy: &Proxy,
    upstream: &Upstream,
    request: Request<Incoming>,
) -> Result<Reply, String> {
    let mut held = upstream.lock().await;
    let ready = match held.as_mut() {
        Some(sender) => sender.ready().await.is_ok(),
        None => false,
    };
    if !ready {
        *held = Some(proxy.target.open().await?);
    }
    let sender = held.as_mut().expect("opened above");
    let response = match sender.try_send_request(request).await {
        Ok(response) => response,
        Err(mut failed) => {
            let Some(request) = failed.take_message() else {
                return Err(failed.into_error().to_string());
            };
            let sender = held.insert(proxy.target.open().await?);
            sender
                .send_request(request)
                .await
                .map_err(|error| error.to_string())?
        }
    };
    Ok(response.map(Either::Left))
}

/// Send an upgrade, such as the console socket, on a connection of its own, and once the Core
/// switches protocols, join the two ends.
async fn upgrade(
    proxy: &Proxy,
    mut request: Request<Incoming>,
) -> Result<Reply, String> {
    let local = hyper::upgrade::on(&mut request);
    let mut sender = proxy.target.open().await?;
    let mut response = sender
        .send_request(request)
        .await
        .map_err(|error| error.to_string())?;
    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        let remote = hyper::upgrade::on(&mut response);
        tokio::spawn(async move {
            let _sender = sender;
            if let (Ok(local), Ok(remote)) = tokio::join!(local, remote) {
                let _ = copy_bidirectional(
                    &mut TokioIo::new(local),
                    &mut TokioIo::new(remote),
                )
                .await;
            }
        });
    }
    Ok(response.map(Either::Left))
}

/// An answer in the shape of Core's own errors, so the App reads both the same way.
fn error(status: StatusCode, code: &str, description: &str) -> Reply {
    let body = serde_json::json!({ "error": code, "description": description });
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Either::Right(Full::from(body.to_string())))
        .expect("a valid response")
}

fn log(request: &Request<Incoming>, webview_authorization: bool) {
    let headers: Vec<String> = request
        .headers()
        .iter()
        .map(|(name, value)| {
            if name == AUTHORIZATION {
                format!("{name}: Bearer <Core session>")
            } else {
                format!("{name}: {}", value.to_str().unwrap_or("<binary>"))
            }
        })
        .collect();
    // Printed rather than traced: it is asked for by name, whatever the log filter is.
    eprintln!(
        "core proxy request {} {} webview_authorization={webview_authorization} headers: {}",
        request.method(),
        request.uri(),
        headers.join(" | "),
    );
}
