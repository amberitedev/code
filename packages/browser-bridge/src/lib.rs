//! Development-only loopback bridge between Chrome and Amberite's real Tauri webview.
//!
use std::{
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
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
use tauri::{AppHandle, Listener, Manager, Runtime, Wry, plugin::TauriPlugin};
use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock},
};
use url::Url;

const ASSET_PATH: &str = "/__amberite_browser_bridge_asset";
const CALLBACK_EVENT: &str = "amberite-browser-bridge://callback";
const SCRIPT_PATH: &str = "/__amberite_browser_bridge.js";
const TAG: &str = "__amberiteBrowserBridge";
const WS_PATH: &str = "/__amberite_browser_bridge_ws";

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
    next_connection_id: AtomicU64,
    sessions: RwLock<HashMap<u64, SocketSink>>,
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
    options: &'a Value,
    response_event: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
struct NativeResponse {
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
                next_connection_id: AtomicU64::new(1),
                sessions: RwLock::new(HashMap::new()),
            });
            register_callback_forwarder(app, Arc::clone(&state));

            tauri::async_runtime::spawn(async move {
                if let Err(error) = serve(state).await {
                    tracing::error!(%error, "Amberite browser bridge stopped");
                }
            });
            Ok(())
        })
        .build()
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
            include_str!("browser-bridge.js"),
        ));
    }
    if path == ASSET_PATH {
        return serve_asset(&request, &state).await;
    }
    if hyper_tungstenite::is_upgrade_request(&request) {
        let target = websocket_target(&state.dev_url, request.uri())?;
        let (response, websocket) = hyper_tungstenite::upgrade(request, None)?;
        tauri::async_runtime::spawn(async move {
            if let Err(error) = proxy_websocket(websocket, target).await {
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
                        invoke_native(
                            &state,
                            &sender,
                            connection_id,
                            id,
                            &cmd,
                            &args,
                            &options,
                        )?;
                    }
                    Ok(IncomingMessage::UnregisterCallback { callback_id }) => {
                        eval_native(
                            &state.app,
                            &format!(
                                "window.__AMBERITE_BROWSER_BRIDGE_NATIVE__?.unregister({connection_id}, {callback_id});"
                            ),
                        )?;
                    }
                    Err(error) => {
                        let payload = wire_error(&format!(
                            "Invalid browser bridge request: {error}"
                        ));
                        send_json(
                            &sender,
                            &json!({
                                "type": "response",
                                "id": Value::Null,
                                "status": "error",
                                "payload": payload,
                            }),
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

    state.sessions.write().await.remove(&connection_id);
    let _ = eval_native(
        &state.app,
        &format!(
            "window.__AMBERITE_BROWSER_BRIDGE_NATIVE__?.clear({connection_id});"
        ),
    );
    Ok(())
}

fn invoke_native(
    state: &Arc<BridgeState>,
    sender: &SocketSink,
    connection_id: u64,
    request_id: u64,
    cmd: &str,
    args: &Value,
    options: &Value,
) -> Result<(), BridgeError> {
    let response_event = format!(
        "amberite-browser-bridge://response/{connection_id}/{request_id}"
    );
    let response_sender = Arc::clone(sender);
    state.app.once_any(response_event.clone(), move |event| {
        let response_sender = Arc::clone(&response_sender);
        let parsed = serde_json::from_str::<NativeResponse>(event.payload());
        tauri::async_runtime::spawn(async move {
            let message = match parsed {
                Ok(response) => json!({
                    "type": "response",
                    "id": request_id,
                    "status": response.status,
                    "payload": response.payload,
                }),
                Err(error) => json!({
                    "type": "response",
                    "id": request_id,
                    "status": "error",
                    "payload": wire_error(&format!("Invalid native response: {error}")),
                }),
            };
            if let Err(error) = send_json(&response_sender, &message).await {
                tracing::warn!(%error, "Browser bridge could not forward an invoke response");
            }
        });
    });

    let request = NativeInvoke {
        args,
        callback_event: CALLBACK_EVENT,
        cmd,
        connection_id,
        options,
        response_event: &response_event,
    };
    let serialized = serde_json::to_string(&request)?;
    let script = format!(
        "{}\nvoid window.__AMBERITE_BROWSER_BRIDGE_NATIVE__.invoke({serialized});",
        include_str!("native-bridge.js")
    );
    eval_native(&state.app, &script)
}

fn eval_native(app: &AppHandle, script: &str) -> Result<(), BridgeError> {
    app.get_webview_window("main")
        .ok_or(BridgeError::MissingWebview)?
        .eval(script)?;
    Ok(())
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

fn inject_bridge(html: &str) -> String {
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
    let injection = format!(
        "<script>window.__AMBERITE_BROWSER_BRIDGE_CONFIG__={config}</script><script src=\"{SCRIPT_PATH}\"></script>"
    );
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
    target: Url,
) -> Result<(), BridgeError> {
    let browser = websocket.await?;
    let (upstream, _) =
        tokio_tungstenite::connect_async(target.as_str()).await?;
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
        assert!(injected.contains("currentWindowLabel"));
        assert!(injected.contains(WS_PATH));
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
    }
}
