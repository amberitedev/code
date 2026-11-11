use axum::{
    Router,
    body::Body,
    http::{
        Request, StatusCode,
        header::{ACCESS_CONTROL_ALLOW_ORIGIN, HOST, ORIGIN, UPGRADE, VARY},
    },
    response::Response,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;

use super::{
    LocalServer,
    endpoints::boot::{Boot, Platform},
    guard::{APP_ORIGINS, Guard},
};

fn boot() -> Boot {
    Boot {
        version: "1.2.3".into(),
        platform: Platform::Linux,
        os_version: "6.1".into(),
        dev: false,
        updates_enabled: false,
        disable_mouseover: false,
        backend_url: None,
    }
}

struct Local {
    router: Router,
    secret: String,
    host: String,
}

fn local() -> Local {
    let server = LocalServer::bind(None, None, None).unwrap();
    Local {
        router: server.router(boot()),
        secret: server.secret.clone(),
        host: format!("127.0.0.1:{}", server.port()),
    }
}

impl Local {
    /// A request to `/app/boot` with a correct secret and Host, plus `headers`.
    async fn boot(&self, headers: &[(&str, &str)]) -> Response {
        let mut request = Request::get(format!("/{}/app/boot", self.secret))
            .header(HOST, &self.host);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        self.send(request.body(Body::empty()).unwrap()).await
    }

    async fn send(&self, request: Request<Body>) -> Response {
        self.router.clone().oneshot(request).await.unwrap()
    }
}

#[tokio::test]
async fn a_request_without_the_secret_is_not_found() {
    let local = local();
    let request = Request::get("/app/boot")
        .header(HOST, &local.host)
        .body(Body::empty())
        .unwrap();
    assert_eq!(local.send(request).await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_foreign_host_is_misdirected() {
    let local = local();
    let request = Request::get(format!("/{}/app/boot", local.secret))
        .header(HOST, "attacker.example:80")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        local.send(request).await.status(),
        StatusCode::MISDIRECTED_REQUEST
    );
}

#[tokio::test]
async fn a_same_origin_request_is_refused() {
    let response = local().boot(&[("sec-fetch-site", "same-origin")]).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_foreign_origin_is_refused() {
    let response = local()
        .boot(&[(ORIGIN.as_str(), "https://example.com")])
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[cfg(not(feature = "browser-bridge"))]
#[tokio::test]
async fn another_loopback_port_is_refused() {
    let response = local()
        .boot(&[(ORIGIN.as_str(), "http://127.0.0.1:1")])
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_websocket_upgrade_without_an_origin_is_refused() {
    let response = local().boot(&[(UPGRADE.as_str(), "websocket")]).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_app_origin_gets_cors_headers() {
    let local = local();
    let response = local.boot(&[(ORIGIN.as_str(), "tauri://localhost")]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
        "tauri://localhost"
    );
    assert_eq!(response.headers()[VARY], "Origin");

    let response = local.boot(&[]).await;
    assert!(!response.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN));
    assert_eq!(response.headers()[VARY], "Origin");

    let preflight = Request::options(format!("/{}/app/boot", local.secret))
        .header(HOST, &local.host)
        .header(ORIGIN, "http://tauri.localhost")
        .header("access-control-request-method", "GET")
        .body(Body::empty())
        .unwrap();
    let response = local.send(preflight).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
        "http://tauri.localhost"
    );
}

#[test]
fn the_dev_origins_are_the_vite_port_on_loopback_names() {
    let guard = Guard::new(
        "secret".into(),
        1,
        Some(&"http://localhost:1420".parse().unwrap()),
    );
    let rows = [
        ("http://localhost:1420", true),
        ("http://scenario-2.localhost:1420", true),
        ("http://127.0.0.1:1420", true),
        ("https://scenario-2.localhost:1420", false),
        ("http://localhost:1421", false),
        ("http://localhost.example.com:1420", false),
        ("null", false),
    ];
    for (origin, allowed) in
        APP_ORIGINS.map(|origin| (origin, true)).iter().chain(&rows)
    {
        assert_eq!(guard.allows(origin), *allowed, "{origin}");
    }
}

#[test]
fn the_init_script_gets_the_base_and_the_main_window_origins() {
    let server = LocalServer::bind(
        None,
        None,
        Some("http://localhost:1420".parse().unwrap()),
    )
    .unwrap();
    let call = format!(
        r#"localInit("{}", ["tauri://localhost","http://tauri.localhost","https://tauri.localhost","http://localhost:1420"]);"#,
        server.base_url()
    );
    assert!(server.init_script().contains(&call));
}

/// No engine exists in this test: boot answers as soon as the listener is bound.
#[tokio::test]
async fn boot_answers_from_bind() {
    let server = LocalServer::bind(None, None, None).unwrap();
    let request = format!(
        "GET /{}/app/boot HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        server.secret,
        server.port()
    );
    let address = server.listener.local_addr().unwrap();
    let router = server.router(boot());
    tokio::spawn(server.serve(router));

    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains(r#""version":"1.2.3""#), "{response}");
}
