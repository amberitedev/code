use axum::{
    Router,
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{
            ACCESS_CONTROL_ALLOW_ORIGIN, CONNECTION, CONTENT_TYPE, HOST,
            ORIGIN, UPGRADE, VARY,
        },
    },
    response::Response,
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;

use super::{
    AppSlot, EngineHandle, LocalServer, Updater,
    endpoints::boot::{Boot, Platform},
    events::Events,
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
    updater: Arc<Updater>,
    secret: String,
    host: String,
    /// The Core proxy's.
    proxy_origin: String,
}

fn local() -> Local {
    local_with(EngineHandle::default())
}

fn local_with(engine: EngineHandle) -> Local {
    let server = LocalServer::bind(None, None, None).unwrap();
    let app = AppSlot::default();
    let updater = Arc::new(Updater::new(app.clone()));
    Local {
        router: server.router(
            boot(),
            Events::default(),
            engine,
            app,
            Arc::clone(&updater),
        ),
        updater,
        secret: server.secret.clone(),
        host: format!("127.0.0.1:{}", server.port()),
        proxy_origin: format!(
            "http://127.0.0.1:{}",
            server.proxy.local_addr().unwrap().port()
        ),
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

    /// A GET from the App's origin to `path` under the secret, plus `headers`.
    fn get(&self, path: &str, headers: &[(&str, &str)]) -> Request<Body> {
        let mut request = Request::get(format!("/{}/{path}", self.secret))
            .header(HOST, &self.host)
            .header(ORIGIN, "tauri://localhost");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request.body(Body::empty()).unwrap()
    }

    async fn send(&self, request: Request<Body>) -> Response {
        self.router.clone().oneshot(request).await.unwrap()
    }
}

#[tokio::test]
async fn a_request_without_the_secret_is_not_found() {
    let local = local();
    for path in ["/app/boot", "/app/file?path=Cargo.toml"] {
        let request = Request::get(path)
            .header(HOST, &local.host)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            local.send(request).await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
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

/// Above all the Core proxy's, which serves other people's content.
#[cfg(not(feature = "browser-bridge"))]
#[tokio::test]
async fn another_loopback_port_is_refused() {
    let local = local();
    let response = local.boot(&[(ORIGIN.as_str(), &local.proxy_origin)]).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// Core content must never share the local server's origin.
#[tokio::test]
async fn cores_are_reached_on_the_proxy_port() {
    let local = local();
    let target = serde_json::json!({
        "core_id": "core",
        "account": "account",
        "address": "https://127.0.0.1:1",
        "fingerprint": "0".repeat(64),
    });
    let request = Request::post(format!("/{}/app/cores/connect", local.secret))
        .header(HOST, &local.host)
        .header(ORIGIN, "tauri://localhost")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(target.to_string()))
        .unwrap();
    let response = local.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let base: String = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        base,
        format!("{}/{}/core/0", local.proxy_origin, local.secret)
    );
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
    let router = server.router(
        boot(),
        Events::default(),
        EngineHandle::default(),
        AppSlot::default(),
        Arc::new(Updater::new(AppSlot::default())),
    );
    tokio::spawn(server.serve(router));

    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains(r#""version":"1.2.3""#), "{response}");
}

/// The socket opens once the engine is up. Then, through the guard and the upgrade, an engine
/// event and an App event raised by engine code arrive as JSON.
#[tokio::test(flavor = "multi_thread")]
async fn the_event_socket_waits_for_the_engine_then_sends_events_as_json() {
    let server = LocalServer::bind(None, None, None).unwrap();
    let request = format!(
        "GET /{}/events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: tauri://localhost\r\n\
         Connection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        server.secret,
        server.port()
    );
    let address = server.listener.local_addr().unwrap();
    let events = Events::default();
    let handle = EngineHandle::default();
    let router = server.router(
        boot(),
        events.clone(),
        handle.clone(),
        AppSlot::default(),
        Arc::new(Updater::new(AppSlot::default())),
    );
    tokio::spawn(server.serve(router));

    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(200), stream.read_u8())
            .await
            .is_err(),
        "the socket answered before the engine started"
    );
    engine::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    // Its own sink, so its startup events stay off this socket.
    handle.set(Ok(engine::Engine::new(engine::EngineConfig {
        settings_dir: dir.path().to_path_buf(),
        events: engine::EventSink::new(),
        accounts: None,
    })
    .await
    .unwrap()));

    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        head.push(stream.read_u8().await.unwrap());
    }
    assert!(
        head.starts_with(b"HTTP/1.1 101"),
        "{}",
        String::from_utf8_lossy(&head)
    );

    engine::emit_warning(&events.engine, "low disk")
        .await
        .unwrap();
    engine::handler::parse_and_emit_command(
        &events.engine,
        "modrinth://mod/sodium",
    )
    .await
    .unwrap();
    for expected in [
        json!({ "type": "warning", "payload": { "message": "low disk" } }),
        json!({ "type": "command", "payload": { "event": "InstallMod", "id": "sodium" } }),
    ] {
        // A short unmasked text frame: FIN and opcode, then the length.
        let [opcode, length] = [
            stream.read_u8().await.unwrap(),
            stream.read_u8().await.unwrap(),
        ];
        assert_eq!(opcode, 0x81);
        let mut payload = vec![0; usize::from(length)];
        stream.read_exact(&mut payload).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&payload).unwrap(),
            expected
        );
    }
}

#[tokio::test]
async fn a_failed_start_answers_engine_unavailable() {
    let handle = EngineHandle::default();
    handle.set(Err(anyhow::anyhow!("the data folder is in use")));
    let local = local_with(handle);
    for path in ["events", "instances", "app/library/recent-icon-configs"] {
        let request = Request::get(format!("/{}/{path}", local.secret))
            .header(HOST, &local.host)
            .header(ORIGIN, "tauri://localhost")
            .body(Body::empty())
            .unwrap();
        let response = local.send(request).await;
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{path}"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({ "error": "engine_unavailable", "description": "the data folder is in use" })
        );
    }
}

/// The page offers to retry or cancel a directory move only when it reads the move's own code.
#[tokio::test]
async fn a_failed_directory_move_keeps_its_code() {
    let handle = EngineHandle::default();
    handle.set(Err(anyhow::Error::new(engine::ErrorKind::NotEnoughSpace {
        required: 1,
    })
    .context(engine::ErrorKind::DirectoryMoveFailed)));
    let local = local_with(handle);
    let response = local.send(local.get("events", &[])).await;
    assert_eq!(response.status(), StatusCode::INSUFFICIENT_STORAGE);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["error"], "directory_move_no_space");
}

/// Opening the event socket retries a failed start, as a reload did when the start was a
/// command; the HTTP read that asks why a socket was refused does not.
#[tokio::test]
async fn opening_the_socket_retries_a_failed_start() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let dir = Arc::new(tempfile::tempdir().unwrap());
    let handle = EngineHandle::default();
    handle.start({
        let attempts = Arc::clone(&attempts);
        move || {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            let dir = Arc::clone(&dir);
            async move {
                anyhow::ensure!(attempt > 0, "the data folder is in use");
                engine::install_crypto_provider();
                Ok(engine::Engine::new(engine::EngineConfig {
                    settings_dir: dir.path().to_path_buf(),
                    events: engine::EventSink::new(),
                    accounts: None,
                })
                .await?)
            }
        }
    });
    let local = local_with(handle);

    let probe = local.send(local.get("events", &[])).await;
    assert_eq!(probe.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(attempts.load(Ordering::SeqCst), 1);

    let socket = local
        .send(local.get(
            "events",
            &[
                (CONNECTION.as_str(), "Upgrade"),
                (UPGRADE.as_str(), "websocket"),
            ],
        ))
        .await;
    // Past the gate: only the upgrade itself fails without a real connection.
    assert_ne!(socket.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    let instances = local.send(local.get("instances", &[])).await;
    assert_eq!(instances.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_screenshot_edit_past_axums_default_body_limit_reaches_the_engine() {
    engine::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    let handle = EngineHandle::default();
    handle.set(Ok(engine::Engine::new(engine::EngineConfig {
        settings_dir: dir.path().to_path_buf(),
        events: engine::EventSink::new(),
        accounts: None,
    })
    .await
    .unwrap()));
    let local = local_with(handle);
    let request = Request::post(format!(
        "/{}/app/screenshots/edits?instance_id=missing&file_name=a.png&mode=create_copy",
        local.secret
    ))
    .header(HOST, &local.host)
    .header(ORIGIN, "tauri://localhost")
    .header(CONTENT_TYPE, "image/png")
    .body(Body::from(vec![0u8; 3 * 1024 * 1024]))
    .unwrap();

    let response = local.send(request).await;

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["description"], "invalid input: unknown instance");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_external_save_into_the_apps_instances_is_refused() {
    engine::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    let engine = engine::Engine::new(engine::EngineConfig {
        settings_dir: dir.path().to_path_buf(),
        events: engine::EventSink::new(),
        accounts: None,
    })
    .await
    .unwrap();
    let path = engine.directories.instances_dir().join("world.zip");
    let handle = EngineHandle::default();
    handle.set(Ok(engine));
    let local = local_with(handle);
    let request =
        Request::post(format!("/{}/app/ops/external-save", local.secret))
            .header(HOST, &local.host)
            .header(ORIGIN, "tauri://localhost")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "op": "save",
                    "path": path,
                    "url": "http://127.0.0.1:9/world.zip",
                    "headers": {},
                })
                .to_string(),
            ))
            .unwrap();

    let response = local.send(request).await;

    assert_ne!(response.status(), StatusCode::ACCEPTED, "the save started");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body["description"],
        "invalid input: save a copy outside the store and instance directories"
    );
}

/// Only a release build finds, downloads and installs a real update. The routes answer without the
/// engine, since an update is how a release that cannot start it gets fixed.
#[tokio::test]
async fn without_the_updater_no_update_is_found_or_downloaded() {
    let handle = EngineHandle::default();
    handle.set(Err(anyhow::anyhow!("the data folder is in use")));
    let local = &local_with(handle);
    let send = |request: axum::http::request::Builder, body: Option<Value>| {
        let request = request
            .header(HOST, &local.host)
            .header(ORIGIN, "tauri://localhost")
            .header(CONTENT_TYPE, "application/json")
            .body(body.map_or_else(Body::empty, |body| body.to_string().into()))
            .unwrap();
        async move {
            let response = local.send(request).await;
            let status = response.status();
            let body =
                to_bytes(response.into_body(), usize::MAX).await.unwrap();
            (status, serde_json::from_slice::<Value>(&body).ok())
        }
    };
    let path = |path: &str| format!("/{}/{path}", local.secret);

    let (_, check) = send(Request::post(path("app/updates/check")), None).await;
    assert_eq!(check.unwrap()["error"], "unsupported");
    let (status, download) = send(
        Request::post(path("app/ops/update-download")),
        Some(json!({ "op": "update" })),
    )
    .await;
    assert_ne!(status, StatusCode::ACCEPTED, "a download op started");
    assert_eq!(download.unwrap()["error"], "not_found");

    send(
        Request::put(path("app/updates/restart-after")),
        Some(json!({ "restart": true })),
    )
    .await;
    assert!(local.updater.restarts_after());
}

#[tokio::test(flavor = "multi_thread")]
async fn with_nobody_signed_in_the_account_reads_are_empty() {
    engine::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    let handle = EngineHandle::default();
    handle.set(Ok(engine::Engine::new(engine::EngineConfig {
        settings_dir: dir.path().to_path_buf(),
        events: engine::EventSink::new(),
        accounts: None,
    })
    .await
    .unwrap()));
    let local = local_with(handle);

    for (path, empty) in [
        ("app/accounts/minecraft", json!([])),
        ("app/accounts/minecraft/default", Value::Null),
        ("app/accounts/modrinth", json!([])),
        ("app/accounts/modrinth/active", Value::Null),
    ] {
        let response = local.send(local.get(path, &[])).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), empty);
    }
}
