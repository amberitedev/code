//! The proxy against stand-in Cores over real connections: what reaches a Core, and what does
//! not.

use std::convert::Infallible;

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::{
    HeaderMap, Method, Request, Response, StatusCode,
    body::Incoming,
    header::{
        ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_METHOD,
        AUTHORIZATION, CONNECTION, CONTENT_SECURITY_POLICY, COOKIE, ORIGIN,
        REFERER, REFERRER_POLICY, UPGRADE, X_CONTENT_TYPE_OPTIONS,
    },
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

use super::*;
use crate::local_api::{endpoints::cores::Reached, guard::Guard};

const APP: &str = "tauri://localhost";
const ACCOUNT_TOKEN: &str = "Bearer mra_the_account_session";
const SECRET: &str =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// What a stand-in Core saw of one request: its path and query, and its headers.
type Seen = (String, HeaderMap);

/// A Core on a random port that reports every request it gets. `/ws` switches protocols and
/// echoes what it is sent. Returns its address, fingerprint, and the reports.
async fn core() -> (String, String, mpsc::UnboundedReceiver<Seen>) {
    let listener = connect::Endpoint::listener(
        (std::net::Ipv4Addr::LOCALHOST, 0).into(),
        connect::Identity::generate().unwrap(),
        &[connect::API],
        connect::Gate::IntroducedOrLocal,
    )
    .unwrap();
    let address = format!("https://{}", listener.local_addr().unwrap());
    let fingerprint = listener.id().to_string();
    let (seen, reports) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(incoming) = listener.accept().await {
            let seen = seen.clone();
            tokio::spawn(async move {
                let connection = incoming.accept().await.unwrap();
                while let Some(stream) = connection.accept().await {
                    tokio::spawn(serve(stream, seen.clone()));
                }
            });
        }
    });
    (address, fingerprint, reports)
}

/// One HTTP connection of a stand-in Core.
async fn serve(stream: connect::Stream, seen: mpsc::UnboundedSender<Seen>) {
    let service = service_fn(move |mut request: Request<Incoming>| {
        let seen = seen.clone();
        async move {
            seen.send((request.uri().to_string(), request.headers().clone()))
                .unwrap();
            if request.uri().path() != "/ws" {
                return Ok::<_, Infallible>(Response::new(
                    Full::<Bytes>::from("ok"),
                ));
            }
            let upgraded = hyper::upgrade::on(&mut request);
            tokio::spawn(async move {
                let mut socket = TokioIo::new(upgraded.await.unwrap());
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
}

/// The App's Core proxy on a random port, with no tunnels yet.
struct Proxy {
    port: u16,
    cores: Arc<Cores>,
}

async fn proxy() -> Proxy {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let cores =
        Arc::new(Cores::new(format!("http://127.0.0.1:{port}/{SECRET}")));
    let guard = Arc::new(Guard::new(SECRET.into(), port, None));
    tokio::spawn(proxy::serve(listener, guard, Arc::clone(&cores)));
    Proxy { port, cores }
}

impl Proxy {
    /// Connects to the Core at `address` signing with `token`; returns the tunnel's path.
    async fn tunnel(
        &self,
        address: &str,
        fingerprint: &str,
        token: Option<&str>,
    ) -> String {
        let key = Key {
            account: "account".into(),
            core_id: address.into(),
        };
        self.cores.sessions.cache(&key, token);
        let route = self
            .cores
            .connect(CoreTarget {
                core_id: key.core_id,
                account: key.account,
                fingerprint: fingerprint.into(),
                address: address.into(),
                introduced: None,
            })
            .await
            .unwrap();
        assert_eq!(route.reached, Some(Reached::Local));
        self.path(&route.base)
    }

    /// The path of a proxy URL.
    fn path(&self, base: &str) -> String {
        base.strip_prefix(&format!("http://127.0.0.1:{}", self.port))
            .unwrap()
            .to_owned()
    }

    /// A request for exactly `path` on the proxy, as the WebView sends it.
    fn get(&self, path: &str) -> hyper::http::request::Builder {
        Request::builder()
            .uri(path)
            .header(HOST, format!("127.0.0.1:{}", self.port))
    }

    /// Send `request` on a connection of its own.
    async fn send(&self, request: Request<Empty<Bytes>>) -> Response<Incoming> {
        let stream =
            TcpStream::connect(("127.0.0.1", self.port)).await.unwrap();
        let (mut sender, connection) =
            hyper::client::conn::http1::handshake(TokioIo::new(stream))
                .await
                .unwrap();
        tokio::spawn(connection.with_upgrades());
        sender.send_request(request).await.unwrap()
    }
}

/// A proxy with one tunnel to a stand-in Core signing with `token`.
async fn one_core(
    token: Option<&str>,
) -> (Proxy, String, mpsc::UnboundedReceiver<Seen>) {
    let (address, fingerprint, reports) = core().await;
    let proxy = proxy().await;
    let tunnel = proxy.tunnel(&address, &fingerprint, token).await;
    (proxy, tunnel, reports)
}

fn assert_sandboxed(response: &Response<Incoming>) {
    assert_eq!(response.headers()[CONTENT_SECURITY_POLICY], "sandbox");
    assert_eq!(response.headers()[X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(response.headers()[REFERRER_POLICY], "no-referrer");
}

#[tokio::test]
async fn the_session_replaces_whatever_the_webview_sent() {
    let (proxy, tunnel, mut reports) = one_core(Some("core-session")).await;

    let response = proxy
        .send(
            proxy
                .get(&format!("{tunnel}/health"))
                .header(ORIGIN, APP)
                .header(AUTHORIZATION, ACCOUNT_TOKEN)
                .header(COOKIE, "set-by=another-local-site")
                .header(
                    REFERER,
                    format!("http://127.0.0.1:{}{tunnel}/", proxy.port),
                )
                .body(Empty::new())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN], APP);
    assert_sandboxed(&response);
    let (path, seen) = reports.recv().await.unwrap();
    assert_eq!(path, "/health", "the secret and tunnel are not passed on");
    assert_eq!(seen[AUTHORIZATION], "Bearer core-session");
    assert_eq!(seen.get_all(AUTHORIZATION).iter().count(), 1);
    assert!(!seen.contains_key(ORIGIN), "Core refuses any Origin");
    assert!(!seen.contains_key(COOKIE));
    assert!(!seen.contains_key(REFERER), "a proxy URL holds the secret");
}

#[tokio::test]
async fn without_a_session_nothing_is_signed() {
    let (proxy, tunnel, mut reports) = one_core(None).await;

    proxy
        .send(
            proxy
                .get(&format!("{tunnel}/health"))
                .header(ORIGIN, APP)
                .header(AUTHORIZATION, ACCOUNT_TOKEN)
                .body(Empty::new())
                .unwrap(),
        )
        .await;
    assert!(!reports.recv().await.unwrap().1.contains_key(AUTHORIZATION));
}

/// An `<img>` or link sends no `Origin`.
#[tokio::test]
async fn a_load_without_an_origin_is_signed() {
    let (proxy, tunnel, mut reports) = one_core(Some("core-session")).await;

    let response = proxy
        .send(
            proxy
                .get(&format!("{tunnel}/icon.png"))
                .header("sec-fetch-site", "cross-site")
                .body(Empty::new())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN));
    assert_sandboxed(&response);
    assert_eq!(
        reports.recv().await.unwrap().1[AUTHORIZATION],
        "Bearer core-session"
    );
}

#[tokio::test]
async fn the_guard_answers_before_anything_is_signed() {
    let (proxy, tunnel, mut reports) = one_core(Some("core-session")).await;
    let wrong = format!("/{}/core/0/health", "0".repeat(SECRET.len()));
    let short = format!("/{}/core/0/health", &SECRET[..SECRET.len() - 1]);
    let socket = |path: &str| {
        proxy
            .get(path)
            .header(CONNECTION, "upgrade")
            .header(UPGRADE, "websocket")
    };
    let health = format!("{tunnel}/health");
    let rows = [
        // Even with the App's Origin: any local process can send that.
        (
            "no secret",
            proxy.get("/core/0/health").header(ORIGIN, APP),
            404,
        ),
        ("wrong secret", proxy.get(&wrong).header(ORIGIN, APP), 404),
        ("short secret", proxy.get(&short).header(ORIGIN, APP), 404),
        (
            "socket without the secret",
            socket("/core/0/ws").header(ORIGIN, APP),
            404,
        ),
        (
            "no tunnel",
            proxy
                .get(&format!("/{SECRET}/core/1/health"))
                .header(ORIGIN, APP),
            404,
        ),
        (
            "not a tunnel",
            proxy.get(&format!("/{SECRET}/health")).header(ORIGIN, APP),
            404,
        ),
        (
            "rebound name",
            Request::builder()
                .uri(&health)
                .header(HOST, format!("attacker.example:{}", proxy.port))
                .header(ORIGIN, APP),
            421,
        ),
        (
            "same origin",
            proxy
                .get(&health)
                .header("sec-fetch-site", "same-origin")
                .header(ORIGIN, APP),
            403,
        ),
        (
            "another origin",
            proxy.get(&health).header(ORIGIN, "https://example.com"),
            403,
        ),
        (
            "socket without an origin",
            socket(&format!("{tunnel}/ws")),
            403,
        ),
        // Not even the App may sign in through the proxy: the answer holds a raw token.
        (
            "sign-in",
            proxy
                .get(&format!("{tunnel}/sessions"))
                .method(Method::POST)
                .header(ORIGIN, APP),
            403,
        ),
    ];
    for (row, request, status) in rows {
        let response = proxy.send(request.body(Empty::new()).unwrap()).await;
        assert_eq!(response.status().as_u16(), status, "{row}");
        assert_sandboxed(&response);
    }
    // Nothing reached the Core: the next report is this request's.
    proxy
        .send(
            proxy
                .get(&health)
                .header(ORIGIN, APP)
                .body(Empty::new())
                .unwrap(),
        )
        .await;
    assert_eq!(reports.recv().await.unwrap().0, "/health");
    assert!(reports.try_recv().is_err());
}

#[tokio::test]
async fn a_preflight_is_answered_and_sandboxed() {
    let (proxy, tunnel, mut reports) = one_core(None).await;

    let response = proxy
        .send(
            proxy
                .get(&format!("{tunnel}/fs/upload"))
                .method(Method::OPTIONS)
                .header(ORIGIN, APP)
                .header(ACCESS_CONTROL_REQUEST_METHOD, "PUT")
                .body(Empty::new())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN], APP);
    assert_sandboxed(&response);
    assert!(reports.try_recv().is_err());
}

#[tokio::test]
async fn a_console_socket_upgrades_with_the_session() {
    let (proxy, tunnel, mut reports) = one_core(Some("core-session")).await;

    let mut response = proxy
        .send(
            proxy
                .get(&format!("{tunnel}/ws"))
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

/// Browsers share connections across the proxy's one origin, so one carries requests to any Core.
#[tokio::test]
async fn one_local_connection_reaches_each_core_with_its_session() {
    let proxy = proxy().await;
    let (one, one_fingerprint, mut one_reports) = core().await;
    let (other, other_fingerprint, mut other_reports) = core().await;
    let to_one = proxy
        .tunnel(&one, &one_fingerprint, Some("one-session"))
        .await;
    let to_other = proxy
        .tunnel(&other, &other_fingerprint, Some("other-session"))
        .await;
    assert_ne!(to_one, to_other);

    let stream = TcpStream::connect(("127.0.0.1", proxy.port)).await.unwrap();
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .unwrap();
    tokio::spawn(connection);
    for to_one_core in [true, false, true] {
        let (tunnel, reports, session) = if to_one_core {
            (&to_one, &mut one_reports, "Bearer one-session")
        } else {
            (&to_other, &mut other_reports, "Bearer other-session")
        };
        let response = sender
            .send_request(
                proxy
                    .get(&format!("{tunnel}/health"))
                    .header(ORIGIN, APP)
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.into_body().collect().await.unwrap();
        // The Core reports before it answers, so its report is already in.
        let (_, seen) = reports.try_recv().expect("reached its own Core");
        assert_eq!(seen[AUTHORIZATION], session);
    }
    assert!(one_reports.try_recv().is_err());
    assert!(other_reports.try_recv().is_err());
}

#[tokio::test]
async fn connecting_again_reuses_the_tunnel() {
    let (address, fingerprint, _reports) = core().await;
    let proxy = proxy().await;
    let tunnel = proxy.tunnel(&address, &fingerprint, None).await;
    assert_eq!(tunnel, format!("/{SECRET}/core/0"));
    assert_eq!(proxy.tunnel(&address, &fingerprint, None).await, tunnel);
}

/// The frontend sends such a request again once the backend introduced it to the Core, which is
/// only safe because nothing was sent.
#[tokio::test]
async fn a_core_with_no_connection_answers_offline() {
    let proxy = proxy().await;
    let key = Key {
        account: "account".into(),
        core_id: "core".into(),
    };
    let core = connect::Identity::generate().unwrap().id();
    let (n, _) = proxy.cores.tunnel_to(key, core);
    let response = proxy
        .send(
            proxy
                .get(&format!("/{SECRET}/core/{n}/servers"))
                .header(ORIGIN, APP)
                .body(Empty::new())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: ApiError = serde_json::from_slice(&body).unwrap();
    assert_eq!(error.error, ErrorCode::CoreOffline);
}

#[tokio::test]
async fn the_query_survives_the_tunnel() {
    let (proxy, tunnel, mut reports) = one_core(None).await;
    let request = proxy
        .get(&format!("{tunnel}/fs/list?path=%2Fworld&page=2"))
        .header(ORIGIN, APP)
        .body(Empty::new())
        .unwrap();
    proxy.send(request).await;
    assert_eq!(
        reports.recv().await.unwrap().0,
        "/fs/list?path=%2Fworld&page=2"
    );
}
