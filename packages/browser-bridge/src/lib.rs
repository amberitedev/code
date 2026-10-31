//! Development-only loopback bridge between Chrome and Amberite's real Tauri webview.
//!
use std::{
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex as StdMutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use futures::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, Response, StatusCode,
    body::{Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
};
use hyper_tungstenite::{HyperWebsocket, tungstenite::Message};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{
    AppHandle, Listener, Manager, Runtime, Webview, Wry,
    plugin::TauriPlugin,
    webview::{PageLoadEvent, PageLoadPayload},
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock, mpsc},
};
use url::Url;

const ASSET_PATH: &str = "/__browser_bridge_asset";
const CALLBACK_EVENT: &str = "browser-bridge://callback";
const RESPONSE_EVENT: &str = "browser-bridge://response";
const SCRIPT_PATH: &str = "/__browser_bridge.js";
const TAG: &str = "__amberiteBrowserBridge";
const WS_PATH: &str = "/__browser_bridge_ws";

/// How long the native webview gets to run a script before it counts as not running.
const EVAL_TIMEOUT: Duration = Duration::from_secs(5);
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(5);
/// An invoke unanswered for this long is reported by the watchdog.
const STUCK_AFTER: Duration = Duration::from_secs(30);
/// Minimum gap between reloads of a native page that has no Tauri, so a dev server that is still
/// down is not hammered.
const HEAL_COOLDOWN: Duration = Duration::from_secs(10);
const PROBE_SCRIPT: &str = "JSON.stringify({ href: location.href, readyState: document.readyState, tauri: typeof window.__TAURI_INTERNALS__, dispatcher: typeof window.__BROWSER_BRIDGE_NATIVE__ })";

type SocketSink =
    Arc<Mutex<futures::stream::SplitSink<BridgeWebsocket, Message>>>;
type BridgeWebsocket =
    hyper_tungstenite::WebSocketStream<TokioIo<hyper::upgrade::Upgraded>>;

#[derive(Debug, thiserror::Error)]
enum BridgeError {
    #[error("browser bridge requires a loopback Tauri dev URL, received {0}")]
    InvalidDevUrl(Url),
    #[error("browser bridge could not find the main Tauri webview")]
    MissingWebview,
    #[error("browser bridge HTTP error: {0}")]
    Http(#[from] hyper::http::Error),
    #[error("browser bridge HTTP body error: {0}")]
    HttpBody(#[from] hyper::Error),
    #[error("browser bridge I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("browser bridge proxy error: {0}")]
    Proxy(#[from] reqwest::Error),
    #[error("browser bridge WebSocket upgrade error: {0}")]
    Protocol(#[from] hyper_tungstenite::tungstenite::error::ProtocolError),
    #[error("browser bridge serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("browser bridge Tauri error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("browser bridge URL error: {0}")]
    Url(#[from] url::ParseError),
    #[error("browser bridge WebSocket error: {0}")]
    Websocket(#[from] hyper_tungstenite::tungstenite::Error),
}

#[derive(Debug)]
struct BridgeState {
    app: AppHandle,
    client: reqwest::Client,
    dev_url: Url,
    last_heal: StdMutex<Option<Instant>>,
    next_connection_id: AtomicU64,
    /// Every invoke sent to the native page and not yet answered, keyed by connection and request
    /// ID. Whoever removes an entry answers its tab, so each invoke gets exactly one response.
    pending: StdMutex<HashMap<(u64, u64), PendingInvoke>>,
    sessions: RwLock<HashMap<u64, SocketSink>>,
}

impl BridgeState {
    /// Never hold this guard across an await or a webview call: the page load hook takes it on
    /// the main thread.
    fn pending(&self) -> MutexGuard<'_, HashMap<(u64, u64), PendingInvoke>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Debug)]
struct PendingInvoke {
    cmd: String,
    since: Instant,
    sink: SocketSink,
    warned: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum IncomingMessage {
    Invoke {
        id: u64,
        cmd: String,
        args: Value,
        options: Value,
    },
    UnregisterCallback {
        callback_id: u64,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeInvoke<'a> {
    args: &'a Value,
    callback_event: &'static str,
    cmd: &'a str,
    connection_id: u64,
    id: u64,
    options: &'a Value,
    response_event: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeResponse {
    connection_id: u64,
    id: u64,
    payload: Value,
    status: RpcStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum RpcStatus {
    Error,
    Success,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeCallback {
    callback_id: u64,
    connection_id: u64,
    payload: Value,
}

/// Installs and starts the development bridge. Call only behind Amberite's dev-only Cargo feature.
pub fn init() -> TauriPlugin<Wry> {
    tauri::plugin::Builder::new("browser-bridge")
        .setup(|app, _api| {
            if !cfg!(debug_assertions) {
                return Ok(());
            }

            let Some(dev_url) = app.config().build.dev_url.clone() else {
                return Err(
                    "Amberite browser bridge requires build.devUrl".into()
                );
            };
            validate_dev_url(&dev_url)?;

            let state = Arc::new(BridgeState {
                app: app.clone(),
                client: reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()?,
                dev_url,
                last_heal: StdMutex::new(None),
                next_connection_id: AtomicU64::new(1),
                pending: StdMutex::new(HashMap::new()),
                sessions: RwLock::new(HashMap::new()),
            });
            app.manage(Arc::clone(&state));
            register_callback_forwarder(app, Arc::clone(&state));
            register_response_forwarder(app, Arc::clone(&state));

            tauri::async_runtime::spawn(watch_pending(Arc::clone(&state)));
            tauri::async_runtime::spawn(async move {
                if let Err(error) = serve(state).await {
                    tracing::error!(%error, "Amberite browser bridge stopped");
                }
            });
            Ok(())
        })
        .on_page_load(reject_on_reload)
        .build()
}

/// A page that starts loading can no longer answer what the previous page was asked, so tell the
/// tabs now instead of leaving them waiting forever.
fn reject_on_reload(webview: &Webview<Wry>, payload: &PageLoadPayload<'_>) {
    if webview.label() != "main" || payload.event() != PageLoadEvent::Started {
        return;
    }
    let Some(state) = webview.try_state::<Arc<BridgeState>>() else {
        return;
    };
    let interrupted = std::mem::take(&mut *state.pending());
    if interrupted.is_empty() {
        return;
    }
    tracing::warn!(
        count = interrupted.len(),
        "Browser bridge failed the invokes the App's native page had not answered before it reloaded"
    );
    for ((_, id), invoke) in interrupted {
        reject(
            invoke.sink,
            id,
            format!("The App page reloaded before it answered {}", invoke.cmd),
        );
    }
}

fn validate_dev_url(url: &Url) -> Result<(), BridgeError> {
    let is_loopback = match url.host() {
        Some(url::Host::Domain("localhost")) => true,
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    if matches!(url.scheme(), "http" | "https") && is_loopback {
        Ok(())
    } else {
        Err(BridgeError::InvalidDevUrl(url.clone()))
    }
}

fn register_callback_forwarder<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<BridgeState>,
) {
    app.listen_any(CALLBACK_EVENT, move |event| {
        let callback = match serde_json::from_str::<NativeCallback>(event.payload()) {
            Ok(callback) => callback,
            Err(error) => {
                tracing::warn!(%error, "Browser bridge received an invalid callback envelope");
                return;
            }
        };
        let state = Arc::clone(&state);
        tauri::async_runtime::spawn(async move {
            let session = state.sessions.read().await.get(&callback.connection_id).cloned();
            if let Some(session) = session {
                let message = json!({
                    "type": "callback",
                    "callbackId": callback.callback_id,
                    "payload": callback.payload,
                });
                if let Err(error) = send_json(&session, &message).await {
                    tracing::warn!(%error, "Browser bridge could not forward a callback");
                }
            }
        });
    });
}

/// Forwards every native answer to the tab that asked. One permanent listener, because a listener
/// registered per request can miss an answer emitted before Tauri finishes registering it.
fn register_response_forwarder<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<BridgeState>,
) {
    app.listen_any(RESPONSE_EVENT, move |event| {
        let response = match serde_json::from_str::<NativeResponse>(event.payload()) {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(%error, "Browser bridge received an invalid response envelope");
                return;
            }
        };
        // Already answered when the page reloaded, the tab left, or the script never reported.
        let Some(invoke) = state
            .pending()
            .remove(&(response.connection_id, response.id))
        else {
            return;
        };
        tauri::async_runtime::spawn(async move {
            let message = json!({
                "type": "response",
                "id": response.id,
                "status": response.status,
                "payload": response.payload,
            });
            if let Err(error) = send_json(&invoke.sink, &message).await {
                tracing::warn!(%error, "Browser bridge could not forward an invoke response");
            }
        });
    });
}

/// Reports invokes the native page has sat on, once each, with what the page looks like.
async fn watch_pending(state: Arc<BridgeState>) {
    let mut interval = tokio::time::interval(WATCHDOG_INTERVAL);
    loop {
        interval.tick().await;
        let stuck = state
            .pending()
            .values_mut()
            .filter(|invoke| {
                !invoke.warned && invoke.since.elapsed() >= STUCK_AFTER
            })
            .map(|invoke| {
                invoke.warned = true;
                (invoke.cmd.clone(), invoke.since.elapsed().as_secs())
            })
            .collect::<Vec<_>>();
        if stuck.is_empty() {
            continue;
        }
        for (cmd, age_secs) in stuck {
            tracing::warn!(
                cmd,
                age_secs,
                "Browser bridge invoke has not been answered"
            );
        }
        probe_native(&state).await;
    }
}

async fn probe_native(state: &BridgeState) {
    let result = match eval_native_result(&state.app, PROBE_SCRIPT) {
        Ok(result) => result,
        Err(error) => {
            tracing::warn!(%error, "Browser bridge could not probe the App's native page");
            return;
        }
    };
    let Some(report) = script_result(result).await else {
        tracing::warn!(
            "Browser bridge: native webview is not executing script"
        );
        return;
    };
    tracing::warn!(%report, "Browser bridge native page state");
    let has_no_tauri = serde_json::from_str::<Value>(&report)
        .is_ok_and(|report| report["tauri"] == "undefined");
    if has_no_tauri {
        heal(state);
    }
}

/// Reloads a native page that has no Tauri, such as the error page left by a reload while the dev
/// server was restarting. Nobody sees that page because the native window is hidden.
fn heal(state: &BridgeState) {
    {
        let mut last_heal = state
            .last_heal
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if last_heal.is_some_and(|at| at.elapsed() < HEAL_COOLDOWN) {
            return;
        }
        *last_heal = Some(Instant::now());
    }
    let navigated = main_webview(&state.app).and_then(|webview| {
        webview.navigate(state.dev_url.clone()).map_err(Into::into)
    });
    match navigated {
        Ok(()) => tracing::warn!(
            url = %state.dev_url,
            "Browser bridge reloaded the App's native page because it had no Tauri"
        ),
        Err(error) => tracing::warn!(
            %error,
            "Browser bridge could not reload the App's native page"
        ),
    }
}

async fn serve(state: Arc<BridgeState>) -> Result<(), BridgeError> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    tracing::info!(url = %format!("http://{address}"), "Amberite browser bridge ready");

    loop {
        let (stream, peer) = listener.accept().await?;
        let state = Arc::clone(&state);
        tauri::async_runtime::spawn(async move {
            let result = http1::Builder::new()
                .serve_connection(
                    TokioIo::new(stream),
                    service_fn(move |request| {
                        handle_request(request, peer, Arc::clone(&state))
                    }),
                )
                .with_upgrades()
                .await;
            if let Err(error) = result {
                tracing::warn!(%error, "Browser bridge HTTP connection failed");
            }
        });
    }
}

async fn handle_request(
    request: Request<Incoming>,
    peer: SocketAddr,
    state: Arc<BridgeState>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let response = if !peer.ip().is_loopback() {
        response(StatusCode::FORBIDDEN, "text/plain", "Forbidden")
    } else {
        route_request(request, state).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "Browser bridge request failed");
            response(StatusCode::BAD_GATEWAY, "text/plain", error.to_string())
        })
    };
    Ok(response)
}

async fn route_request(
    request: Request<Incoming>,
    state: Arc<BridgeState>,
) -> Result<Response<Full<Bytes>>, BridgeError> {
    let path = request.uri().path().to_owned();
    if path == WS_PATH && hyper_tungstenite::is_upgrade_request(&request) {
        let (response, websocket) = hyper_tungstenite::upgrade(request, None)?;
        tauri::async_runtime::spawn(async move {
            if let Err(error) = handle_bridge_socket(websocket, state).await {
                tracing::warn!(%error, "Browser bridge WebSocket session failed");
            }
        });
        return Ok(response);
    }
    if path == SCRIPT_PATH {
        return Ok(response(
            StatusCode::OK,
            "text/javascript; charset=utf-8",
            browser_script(),
        ));
    }
    if path == ASSET_PATH {
        return serve_asset(&request, &state).await;
    }
    if hyper_tungstenite::is_upgrade_request(&request) {
        let target = websocket_target(&state.dev_url, request.uri())?;
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut upstream_request = target.as_str().into_client_request()?;
        if let Some(protocol) = request.headers().get("sec-websocket-protocol")
        {
            upstream_request
                .headers_mut()
                .insert("sec-websocket-protocol", protocol.clone());
        }
        let (upstream, handshake) =
            tokio_tungstenite::connect_async(upstream_request).await?;
        let (mut response, websocket) =
            hyper_tungstenite::upgrade(request, None)?;
        if let Some(protocol) =
            handshake.headers().get("sec-websocket-protocol")
        {
            response
                .headers_mut()
                .insert("sec-websocket-protocol", protocol.clone());
        }
        tauri::async_runtime::spawn(async move {
            if let Err(error) = proxy_websocket(websocket, upstream).await {
                tracing::warn!(%error, "Browser bridge Vite WebSocket proxy failed");
            }
        });
        return Ok(response);
    }
    proxy_http(request, &state).await
}

async fn handle_bridge_socket(
    websocket: HyperWebsocket,
    state: Arc<BridgeState>,
) -> Result<(), BridgeError> {
    let stream = websocket.await?;
    let (sender, mut receiver) = stream.split();
    let sender = Arc::new(Mutex::new(sender));
    let connection_id =
        state.next_connection_id.fetch_add(1, Ordering::Relaxed);
    state
        .sessions
        .write()
        .await
        .insert(connection_id, Arc::clone(&sender));

    // A request that fails is answered with an error; only a dead socket ends the session.
    let result: Result<(), BridgeError> = async {
        while let Some(message) = receiver.next().await {
            match message? {
                Message::Text(text) => {
                    match serde_json::from_str::<IncomingMessage>(&text) {
                        Ok(IncomingMessage::Invoke {
                            id,
                            cmd,
                            args,
                            options,
                        }) => {
                            if let Err(error) = invoke_native(
                                &state,
                                &sender,
                                connection_id,
                                id,
                                &cmd,
                                &args,
                                &options,
                            ) {
                                tracing::warn!(%error, cmd, "Browser bridge could not send an invoke to the App's native page");
                                send_json(
                                    &sender,
                                    &error_response(
                                        json!(id),
                                        &error.to_string(),
                                    ),
                                )
                                .await?;
                            }
                        }
                        Ok(IncomingMessage::UnregisterCallback {
                            callback_id,
                        }) => {
                            if let Err(error) = eval_native(
                                &state.app,
                                &format!(
                                    "window.__BROWSER_BRIDGE_NATIVE__?.unregister({connection_id}, {callback_id});"
                                ),
                            ) {
                                tracing::warn!(%error, "Browser bridge could not unregister a callback");
                            }
                        }
                        Err(error) => {
                            send_json(
                                &sender,
                                &error_response(
                                    Value::Null,
                                    &format!(
                                        "Invalid browser bridge request: {error}"
                                    ),
                                ),
                            )
                            .await?;
                        }
                    }
                }
                Message::Ping(payload) => {
                    sender.lock().await.send(Message::Pong(payload)).await?
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        Ok(())
    }
    .await;

    state.sessions.write().await.remove(&connection_id);
    // The tab is gone, so there is nobody left to answer.
    state
        .pending()
        .retain(|(connection, _), _| *connection != connection_id);
    let _ = eval_native(
        &state.app,
        &format!("window.__BROWSER_BRIDGE_NATIVE__?.clear({connection_id});"),
    );
    result
}

/// Sends an invoke to the native page. The answer arrives later through the response forwarder; if
/// the page cannot run the invoke at all, the tab is told so here instead of waiting forever.
fn invoke_native(
    state: &Arc<BridgeState>,
    sender: &SocketSink,
    connection_id: u64,
    request_id: u64,
    cmd: &str,
    args: &Value,
    options: &Value,
) -> Result<(), BridgeError> {
    let request = NativeInvoke {
        args,
        callback_event: CALLBACK_EVENT,
        cmd,
        connection_id,
        id: request_id,
        options,
        response_event: RESPONSE_EVENT,
    };
    let serialized = serde_json::to_string(&request)?;
    // The dispatcher is sent with every invoke because a reloaded page has lost it. The script
    // evaluates to what `dispatch` returns.
    let script = format!(
        "{}\nwindow.__BROWSER_BRIDGE_NATIVE__.dispatch({serialized})",
        include_str!("native-bridge.js")
    );

    let key = (connection_id, request_id);
    state.pending().insert(
        key,
        PendingInvoke {
            cmd: cmd.to_owned(),
            since: Instant::now(),
            sink: Arc::clone(sender),
            warned: false,
        },
    );
    let result = match eval_native_result(&state.app, &script) {
        Ok(result) => result,
        Err(error) => {
            state.pending().remove(&key);
            return Err(error);
        }
    };

    let state = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        let outcome = script_result(result).await;
        if outcome.as_deref() == Some("ok") {
            return;
        }
        // Already answered if the page reloaded or the answer beat this check.
        let Some(invoke) = state.pending().remove(&key) else {
            return;
        };
        tracing::warn!(
            cmd = invoke.cmd,
            ?outcome,
            "Browser bridge invoke did not reach a running native page"
        );
        reject(
            invoke.sink,
            request_id,
            "The App's native page is not running (reloading or failed to load)"
                .to_owned(),
        );
        if outcome.as_deref() == Some("no-tauri") {
            heal(&state);
        }
    });
    Ok(())
}

// The upstream ad view shares the main window, so it is no longer a
// single-webview window. Address the launcher webview directly.
fn main_webview(app: &AppHandle) -> Result<Webview<Wry>, BridgeError> {
    app.get_webview("main").ok_or(BridgeError::MissingWebview)
}

fn eval_native(app: &AppHandle, script: &str) -> Result<(), BridgeError> {
    main_webview(app)?.eval(script)?;
    Ok(())
}

/// Runs a script in the native page and returns where its value will arrive. Read it with
/// `script_result`.
fn eval_native_result(
    app: &AppHandle,
    script: &str,
) -> Result<mpsc::UnboundedReceiver<String>, BridgeError> {
    let (sender, receiver) = mpsc::unbounded_channel();
    main_webview(app)?.eval_with_callback(script, move |value| {
        let _ = sender.send(value);
    })?;
    Ok(receiver)
}

/// The string a script evaluated to, or `None` when the webview did not run it in time. The
/// webview reports values as JSON, so a string arrives quoted.
async fn script_result(
    mut result: mpsc::UnboundedReceiver<String>,
) -> Option<String> {
    let value = tokio::time::timeout(EVAL_TIMEOUT, result.recv())
        .await
        .ok()
        .flatten()?;
    Some(serde_json::from_str::<String>(&value).unwrap_or(value))
}

fn reject(sink: SocketSink, id: u64, message: String) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) =
            send_json(&sink, &error_response(json!(id), &message)).await
        {
            tracing::warn!(%error, "Browser bridge could not report a failed invoke");
        }
    });
}

fn error_response(id: Value, message: &str) -> Value {
    json!({
        "type": "response",
        "id": id,
        "status": "error",
        "payload": wire_error(message),
    })
}

async fn send_json(
    sender: &SocketSink,
    value: &Value,
) -> Result<(), BridgeError> {
    sender
        .lock()
        .await
        .send(Message::Text(serde_json::to_string(value)?.into()))
        .await?;
    Ok(())
}

fn wire_error(message: &str) -> Value {
    json!({ TAG: "error", "message": message, "name": "Error" })
}

async fn serve_asset(
    request: &Request<Incoming>,
    state: &BridgeState,
) -> Result<Response<Full<Bytes>>, BridgeError> {
    let params = url::form_urlencoded::parse(
        request.uri().query().unwrap_or_default().as_bytes(),
    )
    .into_owned()
    .collect::<HashMap<_, _>>();
    if params
        .get("protocol")
        .map(String::as_str)
        .unwrap_or("asset")
        != "asset"
    {
        return Ok(response(
            StatusCode::BAD_REQUEST,
            "text/plain",
            "Only the asset protocol is supported",
        ));
    }
    let Some(raw_path) = params.get("path") else {
        return Ok(response(
            StatusCode::BAD_REQUEST,
            "text/plain",
            "Missing asset path",
        ));
    };
    let path = PathBuf::from(raw_path);
    if !state.app.asset_protocol_scope().is_allowed(&path) {
        return Ok(response(
            StatusCode::FORBIDDEN,
            "text/plain",
            "Asset path is outside Tauri's scope",
        ));
    }
    let bytes = tokio::fs::read(&path).await?;
    let mime = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();
    Ok(response(StatusCode::OK, &mime, Bytes::from(bytes)))
}

async fn proxy_http(
    request: Request<Incoming>,
    state: &BridgeState,
) -> Result<Response<Full<Bytes>>, BridgeError> {
    let target = state.dev_url.join(
        request
            .uri()
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/"),
    )?;
    let method =
        reqwest::Method::from_bytes(request.method().as_str().as_bytes())
            .expect("Hyper methods are valid Reqwest methods");
    let mut upstream = state.client.request(method, target);
    for (name, value) in request.headers() {
        if !is_hop_by_hop(name.as_str())
            && name.as_str() != "host"
            && name.as_str() != "accept-encoding"
        {
            upstream = upstream.header(name, value);
        }
    }
    upstream = upstream.header("accept-encoding", "identity");
    let body = request.into_body().collect().await?.to_bytes();
    let upstream = upstream.body(body).send().await?;
    let status = StatusCode::from_u16(upstream.status().as_u16())
        .expect("valid upstream status");
    let headers = upstream.headers().clone();
    let is_html = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    let mut body = upstream.bytes().await?.to_vec();
    if is_html {
        body = inject_bridge(&String::from_utf8_lossy(&body)).into_bytes();
    }

    let mut response = Response::builder().status(status);
    for (name, value) in &headers {
        if !is_hop_by_hop(name.as_str())
            && !matches!(name.as_str(), "content-length" | "content-encoding")
        {
            response = response.header(name, value);
        }
    }
    Ok(response.body(Full::new(Bytes::from(body)))?)
}

fn browser_script() -> String {
    let config = json!({
        "assetPath": ASSET_PATH,
        "currentWebviewLabel": "main",
        "currentWindowLabel": "main",
        "pathDelimiter": if cfg!(windows) { ";" } else { ":" },
        "pathSeparator": if cfg!(windows) { "\\" } else { "/" },
        "os": {
            "arch": tauri_plugin_os::arch(),
            "eol": if cfg!(windows) { "\r\n" } else { "\n" },
            "exeExtension": tauri_plugin_os::exe_extension(),
            "family": tauri_plugin_os::family(),
            "platform": tauri_plugin_os::platform(),
            "type": tauri_plugin_os::type_().to_string(),
            "version": tauri_plugin_os::version().to_string(),
        },
        "wsPath": WS_PATH,
    });
    // Keep configuration in the same-origin script so the upstream policy can
    // continue blocking inline scripts in the browser preview.
    format!(
        "window.__BROWSER_BRIDGE_CONFIG__={config};\n{}",
        include_str!("browser-bridge.js")
    )
}

fn inject_bridge(html: &str) -> String {
    let injection = format!("<script src=\"{SCRIPT_PATH}\"></script>");
    if let Some(index) = html.find("<head>") {
        let insert_at = index + "<head>".len();
        format!("{}{}{}", &html[..insert_at], injection, &html[insert_at..])
    } else {
        format!("{injection}{html}")
    }
}

fn websocket_target(
    dev_url: &Url,
    uri: &hyper::Uri,
) -> Result<Url, BridgeError> {
    let mut target = dev_url.join(
        uri.path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/"),
    )?;
    target
        .set_scheme(if dev_url.scheme() == "https" {
            "wss"
        } else {
            "ws"
        })
        .map_err(|()| BridgeError::InvalidDevUrl(dev_url.clone()))?;
    Ok(target)
}

async fn proxy_websocket(
    websocket: HyperWebsocket,
    upstream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<(), BridgeError> {
    let browser = websocket.await?;
    let (mut browser_sender, mut browser_receiver) = browser.split();
    let (mut upstream_sender, mut upstream_receiver) = upstream.split();

    loop {
        tokio::select! {
            message = browser_receiver.next() => match message {
                Some(Ok(message)) => upstream_sender.send(message).await?,
                Some(Err(error)) => return Err(error.into()),
                None => break,
            },
            message = upstream_receiver.next() => match message {
                Some(Ok(message)) => browser_sender.send(message).await?,
                Some(Err(error)) => return Err(error.into()),
                None => break,
            },
        }
    }
    Ok(())
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn response(
    status: StatusCode,
    content_type: &str,
    body: impl Into<Bytes>,
) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", content_type)
        .header("cache-control", "no-store")
        .body(Full::new(body.into()))
        .expect("static browser bridge response is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_loopback_development_urls() {
        assert!(
            validate_dev_url(&Url::parse("http://localhost:1420").unwrap())
                .is_ok()
        );
        assert!(
            validate_dev_url(&Url::parse("http://127.0.0.1:1420").unwrap())
                .is_ok()
        );
        assert!(
            validate_dev_url(&Url::parse("http://[::1]:1420").unwrap()).is_ok()
        );
        assert!(
            validate_dev_url(&Url::parse("https://example.com").unwrap())
                .is_err()
        );
        assert!(
            validate_dev_url(&Url::parse("file:///tmp/app").unwrap()).is_err()
        );
    }

    #[test]
    fn injects_bridge_before_the_frontend_modules() {
        let html = "<!doctype html><html><head><script type=\"module\" src=\"/src.ts\"></script></head></html>";
        let injected = inject_bridge(html);
        assert!(
            injected.find(SCRIPT_PATH).unwrap()
                < injected.find("/src.ts").unwrap()
        );
        assert!(!injected.contains("<script>"));
        let script = browser_script();
        assert!(script.contains("currentWindowLabel"));
        assert!(script.contains(WS_PATH));
    }

    #[test]
    fn websocket_proxy_never_changes_the_upstream_host() {
        let dev_url = Url::parse("http://127.0.0.1:1420").unwrap();
        let uri = "/vite-hmr?token=secret".parse::<hyper::Uri>().unwrap();
        let target = websocket_target(&dev_url, &uri).unwrap();
        assert_eq!(
            target.as_str(),
            "ws://127.0.0.1:1420/vite-hmr?token=secret"
        );
    }

    #[test]
    fn native_dispatcher_is_the_exact_tested_asset() {
        let script = include_str!("native-bridge.js");
        assert!(script.contains("request.options"));
        assert!(script.contains("__CHANNEL__"));
        assert!(script.contains("arrayBuffer"));
        assert!(script.contains("callbackEvent"));
        assert!(script.contains("'no-tauri'"));
    }
}
