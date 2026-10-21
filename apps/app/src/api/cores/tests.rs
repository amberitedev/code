//! The proxy against a stand-in Core over real pinned TLS: what reaches the Core, and what does
//! not.

use std::{convert::Infallible, sync::Arc};

use http_body_util::{BodyExt, Empty};
use hyper::{
    HeaderMap, Response,
    body::Incoming,
    header::{
        ACCESS_CONTROL_ALLOW_ORIGIN, CONNECTION, COOKIE, ORIGIN, UPGRADE,
    },
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use rustls::{
    ServerConfig,
    crypto::aws_lc_rs::default_provider,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tokio_rustls::TlsAcceptor;

use super::*;

const APP: &str = "tauri://localhost";
const ACCOUNT_TOKEN: &str = "Bearer mra_the_account_session";
/// The proxies' capability in these tests. A real one comes from `proxy::capability`.
const CAPABILITY: &str =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// What the stand-in Core saw of one request: its path and query, and its headers.
type Seen = (String, HeaderMap);

/// A Core on a random port that reports every request it gets. `/ws` switches protocols and
/// echoes what it is sent. Returns its address, fingerprint, and the reports.
async fn core() -> (String, String, mpsc::UnboundedReceiver<Seen>) {
    let made = rcgen::generate_simple_self_signed(vec!["amberite-core".into()])
        .unwrap();
    let cert = made.cert.der().to_vec();
    let fingerprint = Sha256::digest(&cert)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let config =
        ServerConfig::builder_with_provider(Arc::new(default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert)],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                    made.signing_key.serialize_der(),
                )),
            )
            .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = format!("https://{}", listener.local_addr().unwrap());
    let (seen, reports) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            let seen = seen.clone();
            tokio::spawn(async move {
                let stream = acceptor.accept(stream).await.unwrap();
                let service =
                    service_fn(move |mut request: Request<Incoming>| {
                        let seen = seen.clone();
                        async move {
                            seen.send((
                                request.uri().to_string(),
                                request.headers().clone(),
                            ))
                            .unwrap();
                            if request.uri().path() != "/ws" {
                                return Ok::<_, Infallible>(Response::new(
                                    Full::<Bytes>::from("ok"),
                                ));
                            }
                            let upgraded = hyper::upgrade::on(&mut request);
                            tokio::spawn(async move {
                                let mut socket =
                                    TokioIo::new(upgraded.await.unwrap());
                                let mut buffer = [0; 4];
                                socket.read_exact(&mut buffer).await.unwrap();
                                socket.write_all(&buffer).await.unwrap();
                            });
                            Ok(Response::builder()
                                .status(StatusCode::SWITCHING_PROTOCOLS)
                                .header(CONNECTION, "upgrade")
                                .header(UPGRADE, "websocket")
                                .body(Full::default())
                                .unwrap())
                        }
                    });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .with_upgrades()
                    .await;
            });
        }
    });
    (address, fingerprint, reports)
}

/// A proxy to `address` signing with `token`, and its local port.
async fn proxy(address: &str, fingerprint: &str, token: Option<&str>) -> u16 {
    let key = Key {
        account: "account".into(),
        core_id: "core".into(),
    };
    let proxy = Arc::new(Proxy {
        target: Target::new(address, fingerprint).unwrap(),
        sessions: Arc::new(Sessions::cached(&key, token)),
        key,
        capability: CAPABILITY.into(),
    });
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(proxy::serve(listener, proxy));
    port
}

/// Send `request` to the proxy as the WebView would: plain HTTP/1.1 on loopback.
async fn send(port: u16, request: Request<Empty<Bytes>>) -> Response<Incoming> {
    let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .unwrap();
    tokio::spawn(connection.with_upgrades());
    sender.send_request(request).await.unwrap()
}

/// A request to `path` on the Core, through the proxy's capability, as the WebView sends it.
fn get(port: u16, path: &str) -> hyper::http::request::Builder {
    bare(port, &format!("/{CAPABILITY}{path}"))
}

/// A request for exactly `path` on the proxy, capability or not.
fn bare(port: u16, path: &str) -> hyper::http::request::Builder {
    Request::builder()
        .uri(path)
        .header(HOST, format!("127.0.0.1:{port}"))
}

#[tokio::test]
async fn the_session_replaces_whatever_the_webview_sent() {
    let (address, fingerprint, mut reports) = core().await;
    let port = proxy(&address, &fingerprint, Some("core-session")).await;

    let response = send(
        port,
        get(port, "/health")
            .header(ORIGIN, APP)
            .header(AUTHORIZATION, ACCOUNT_TOKEN)
            .header(COOKIE, "set-by=another-local-site")
            .body(Empty::new())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN], APP);
    let (path, seen) = reports.recv().await.unwrap();
    assert_eq!(path, "/health", "the capability is not passed on");
    assert_eq!(seen[AUTHORIZATION], "Bearer core-session");
    assert_eq!(seen.get_all(AUTHORIZATION).iter().count(), 1);
    assert!(!seen.contains_key(ORIGIN), "Core refuses any Origin");
    assert!(!seen.contains_key(COOKIE));
}

#[tokio::test]
async fn without_a_session_nothing_is_signed() {
    let (address, fingerprint, mut reports) = core().await;
    let port = proxy(&address, &fingerprint, None).await;

    send(
        port,
        get(port, "/health")
            .header(ORIGIN, APP)
            .header(AUTHORIZATION, ACCOUNT_TOKEN)
            .body(Empty::new())
            .unwrap(),
    )
    .await;
    assert!(!reports.recv().await.unwrap().1.contains_key(AUTHORIZATION));
}

#[tokio::test]
async fn only_the_app_can_use_the_proxy() {
    let (address, fingerprint, mut reports) = core().await;
    let port = proxy(&address, &fingerprint, Some("core-session")).await;

    for origin in [None, Some("https://example.com")] {
        let mut request = get(port, "/sessions");
        if let Some(origin) = origin {
            request = request.header(ORIGIN, origin);
        }
        let response = send(port, request.body(Empty::new()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    // Not even the App may sign in through the proxy: the answer holds a raw token.
    let sign_in = get(port, "/sessions")
        .method(Method::POST)
        .header(ORIGIN, APP)
        .body(Empty::new())
        .unwrap();
    assert_eq!(send(port, sign_in).await.status(), StatusCode::FORBIDDEN);
    // Nothing reached the Core: the next report is this request's.
    send(
        port,
        get(port, "/health")
            .header(ORIGIN, APP)
            .body(Empty::new())
            .unwrap(),
    )
    .await;
    let seen = reports.recv().await.unwrap().1;
    assert_eq!(seen[AUTHORIZATION], "Bearer core-session");
    assert!(reports.try_recv().is_err());
}

#[tokio::test]
async fn a_console_socket_upgrades_with_the_session() {
    let (address, fingerprint, mut reports) = core().await;
    let port = proxy(&address, &fingerprint, Some("core-session")).await;

    let mut response = send(
        port,
        get(port, "/ws")
            .header(ORIGIN, APP)
            .header(CONNECTION, "upgrade")
            .header(UPGRADE, "websocket")
            .body(Empty::new())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    assert_eq!(
        reports.recv().await.unwrap().1[AUTHORIZATION],
        "Bearer core-session"
    );
    let mut socket =
        TokioIo::new(hyper::upgrade::on(&mut response).await.unwrap());
    socket.write_all(b"ping").await.unwrap();
    let mut echoed = [0; 4];
    socket.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"ping");
}

#[tokio::test]
async fn one_local_connection_carries_several_requests() {
    let (address, fingerprint, mut reports) = core().await;
    let port = proxy(&address, &fingerprint, None).await;
    let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .unwrap();
    tokio::spawn(connection);
    for _ in 0..3 {
        let response = sender
            .send_request(
                get(port, "/health")
                    .header(ORIGIN, APP)
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.into_body().collect().await.unwrap();
        reports.recv().await.unwrap().1;
    }
}

#[tokio::test]
async fn without_the_capability_nothing_is_signed_or_sent() {
    let (address, fingerprint, mut reports) = core().await;
    let port = proxy(&address, &fingerprint, Some("core-session")).await;

    let wrong = format!("/{}/health", "0".repeat(CAPABILITY.len()));
    let short = format!("/{}/health", &CAPABILITY[..CAPABILITY.len() - 1]);
    let longer = format!("/{CAPABILITY}0/health");
    for path in [
        "/health",
        "/",
        wrong.as_str(),
        short.as_str(),
        longer.as_str(),
    ] {
        // Even with the App's Origin: any local process can send that.
        let response = send(
            port,
            bare(port, path)
                .header(ORIGIN, APP)
                .body(Empty::new())
                .unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    let upgrade = bare(port, "/ws")
        .header(ORIGIN, APP)
        .header(CONNECTION, "upgrade")
        .header(UPGRADE, "websocket")
        .body(Empty::new())
        .unwrap();
    assert_eq!(send(port, upgrade).await.status(), StatusCode::NOT_FOUND);
    // Nothing reached the Core.
    send(
        port,
        get(port, "/health")
            .header(ORIGIN, APP)
            .body(Empty::new())
            .unwrap(),
    )
    .await;
    assert_eq!(reports.recv().await.unwrap().0, "/health");
    assert!(reports.try_recv().is_err());
}

#[tokio::test]
async fn the_query_survives_the_capability() {
    let (address, fingerprint, mut reports) = core().await;
    let port = proxy(&address, &fingerprint, None).await;
    let request = get(port, "/fs/list?path=%2Fworld&page=2")
        .header(ORIGIN, APP)
        .body(Empty::new())
        .unwrap();
    send(port, request).await;
    assert_eq!(
        reports.recv().await.unwrap().0,
        "/fs/list?path=%2Fworld&page=2"
    );
}

#[test]
fn each_capability_is_32_fresh_random_bytes() {
    let (one, other) = (proxy::capability(), proxy::capability());
    assert_eq!(one.len(), 64);
    assert!(one.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(one, other);
}
