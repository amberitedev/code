//! The App's Core proxy: one HTTP/1.1 listener on 127.0.0.1 for every paired Core. A request to
//! `/<secret>/core/<n>/<path>` goes on to `<path>` on the Core behind tunnel `n` (see
//! `Cores::connect`) over pinned TLS, signed with this device's session there.
//!
//! The WebView's own `Authorization` is dropped, so the account session can never reach a Core,
//! and the session token is added here, so the WebView never holds it. Console sockets are
//! upgraded through the proxy too: a browser `WebSocket` cannot set headers at all.
//!
//! It is guarded like the local server (`server::guard`): the App's secret path segment, a loopback
//! `Host`, no same-origin callers, only the App's origins, and an origin on every socket. A request
//! that fails any of these is answered here and nothing is signed. This matters because a Core
//! often runs on the same PC, where a mod runs as another OS user.
//!
//! It listens on a port of its own so that content from other people's Cores never shares an
//! origin with the local server, whose routes read this computer's files, and the local server
//! refuses its origin. Every answer carries `Content-Security-Policy: sandbox`, `nosniff` and
//! `Referrer-Policy: no-referrer`, so a Core's file opened as a document runs no script, gets an
//! opaque origin and never sends its URL, which holds the secret. Its `<img>` and links can still
//! reach another tunnel by a relative URL, signed, but it can read no answer.

use std::{
    collections::HashMap, convert::Infallible, sync::Arc, sync::LazyLock,
    time::Duration,
};

use axum::{
    body::Body,
    response::{IntoResponse, Response},
};
use contracts::{ApiError, ErrorCode};
use hyper::{
    Method, Request, StatusCode, Uri,
    body::Incoming,
    client::conn::http1::SendRequest,
    header::{
        AUTHORIZATION, CONTENT_SECURITY_POLICY, COOKIE, HeaderValue, ORIGIN,
        REFERER, REFERRER_POLICY, UPGRADE, X_CONTENT_TYPE_OPTIONS,
    },
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::{io::copy_bidirectional, net::TcpListener, sync::Mutex};

use super::{Cores, Tunnel};
use crate::server::guard::{self, Guard};

/// Set to print the head of every request the proxy sends to a Core, with the session token left
/// out, and whether the WebView had sent an `Authorization` of its own.
const LOG_VAR: &str = "CORE_PROXY_LOG";
static LOG: LazyLock<bool> =
    LazyLock::new(|| std::env::var_os(LOG_VAR).is_some());

/// This local connection's connections to Cores, by tunnel, each opened on its first request.
type Upstream = Arc<Mutex<HashMap<usize, SendRequest<Incoming>>>>;

/// Serve `listener` for as long as the App runs. `guard` names the listener's own port.
pub async fn serve(
    listener: TcpListener,
    guard: Arc<Guard>,
    cores: Arc<Cores>,
) {
    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            // Such as running out of sockets: every Core is behind this listener, so keep it.
            Err(problem) => {
                tracing::debug!(%problem, "the Core proxy missed a connection");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let (guard, cores) = (Arc::clone(&guard), Arc::clone(&cores));
        tokio::spawn(async move {
            let upstream = Upstream::default();
            let service = service_fn(move |request| {
                let (guard, cores, upstream) = (
                    Arc::clone(&guard),
                    Arc::clone(&cores),
                    Arc::clone(&upstream),
                );
                async move {
                    let mut response =
                        handle(&guard, &cores, &upstream, request).await;
                    let headers = response.headers_mut();
                    headers.insert(
                        CONTENT_SECURITY_POLICY,
                        HeaderValue::from_static("sandbox"),
                    );
                    headers.insert(
                        X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    );
                    headers.insert(
                        REFERRER_POLICY,
                        HeaderValue::from_static("no-referrer"),
                    );
                    Ok::<_, Infallible>(response)
                }
            });
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .with_upgrades()
                .await;
        });
    }
}

async fn handle(
    guard: &Guard,
    cores: &Cores,
    upstream: &Upstream,
    request: Request<Incoming>,
) -> Response {
    let origin = match guard.screen(
        request.method(),
        request.uri().path(),
        request.headers(),
    ) {
        Ok(origin) => origin,
        Err(answer) => return answer,
    };
    let mut response = pass_on(cores, upstream, request).await;
    guard::allow(response.headers_mut(), origin);
    response
}

/// Sends a request the guard let through to its Core, signed.
async fn pass_on(
    cores: &Cores,
    upstream: &Upstream,
    mut request: Request<Incoming>,
) -> Response {
    let Some((n, uri)) = on_core(request.uri()) else {
        return error(ErrorCode::NotFound, "Not found.");
    };
    let Some(tunnel) = cores.tunnel(n) else {
        return error(ErrorCode::NotFound, "Not found.");
    };
    *request.uri_mut() = uri;
    // A session is only ever made through the App's sign-in route, which keeps its token out of
    // the WebView. Here the answer would hand the WebView a raw token.
    if request.method() == Method::POST && request.uri().path() == "/sessions" {
        return error(
            ErrorCode::Forbidden,
            "Sign in to a Core through the App.",
        );
    }

    let headers = request.headers_mut();
    let webview_authorization = headers.remove(AUTHORIZATION).is_some();
    // Core refuses any request with an `Origin`: only a browser sends one. Cookies are the
    // browser's for every port on this computer, and Core uses none. A `Referer` can be a proxy
    // URL, which holds the secret.
    headers.remove(ORIGIN);
    headers.remove(COOKIE);
    headers.remove(REFERER);
    if let Some(token) = cores.sessions.token(&tunnel.key).await
        && let Ok(value) = HeaderValue::try_from(format!("Bearer {token}"))
    {
        headers.insert(AUTHORIZATION, value);
    }
    if *LOG {
        log(&request, webview_authorization);
    }

    let sent = if request.headers().contains_key(UPGRADE) {
        upgrade(&tunnel, request).await
    } else {
        forward(&tunnel, n, upstream, request).await
    };
    sent.unwrap_or_else(|problem| {
        tracing::debug!(%problem, "a Core did not answer");
        error(ErrorCode::NetworkError, "The Core did not answer.")
    })
}

/// The tunnel and the path on its Core, from `/<secret>/core/<n>/<path>`. The guard checked the
/// secret.
fn on_core(uri: &Uri) -> Option<(usize, Uri)> {
    let mut segments = uri.path().splitn(4, '/').skip(2);
    if segments.next()? != "core" {
        return None;
    }
    let rest = segments.next()?;
    let (n, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path_and_query = match uri.query() {
        Some(query) => format!("/{path}?{query}"),
        None => format!("/{path}"),
    };
    Some((n.parse().ok()?, Uri::try_from(path_and_query).ok()?))
}

/// Send a request on this local connection's connection to the tunnel's Core, opening it first if
/// there is none or it closed. A request the closed connection never sent goes on a new one.
async fn forward(
    tunnel: &Tunnel,
    n: usize,
    upstream: &Upstream,
    request: Request<Incoming>,
) -> Result<Response, String> {
    let mut held = upstream.lock().await;
    let ready = match held.get_mut(&n) {
        Some(sender) => sender.ready().await.is_ok(),
        None => false,
    };
    if !ready {
        held.insert(n, tunnel.target.open().await?);
    }
    let sender = held.get_mut(&n).expect("opened above");
    let response = match sender.try_send_request(request).await {
        Ok(response) => response,
        Err(mut failed) => {
            let Some(request) = failed.take_message() else {
                return Err(failed.into_error().to_string());
            };
            let mut sender = tunnel.target.open().await?;
            let response = sender
                .send_request(request)
                .await
                .map_err(|error| error.to_string())?;
            held.insert(n, sender);
            response
        }
    };
    Ok(response.map(Body::new))
}

/// Send an upgrade, such as the console socket, on a connection of its own, and once the Core
/// switches protocols, join the two ends.
async fn upgrade(
    tunnel: &Tunnel,
    mut request: Request<Incoming>,
) -> Result<Response, String> {
    let local = hyper::upgrade::on(&mut request);
    let mut sender = tunnel.target.open().await?;
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
    Ok(response.map(Body::new))
}

fn error(code: ErrorCode, description: &str) -> Response {
    ApiError::new(code, description).into_response()
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
