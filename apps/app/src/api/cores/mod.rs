//! Paired Cores: pinned connections, and this device's session on each.
//!
//! A Core serves HTTPS with a certificate it made itself, so the WebView cannot talk to it
//! directly. For each Core the App opens a proxy on this computer (see `proxy`) that carries every
//! request to the Core over TLS pinned to the fingerprint the App learned at pairing, and signs it
//! with this device's session there. Requests, uploads, and console sockets all go through that
//! one local address.
//!
//! A session comes from a ticket: the WebView gets one from the backend and hands it to
//! `core_sign_in`, which trades it with the Core for a session token and keeps the token in the
//! OS keychain (see `sessions`). No command returns a token.

mod proxy;
mod sessions;
mod tls;

use std::{collections::HashMap, sync::Arc};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    Method, Request, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, HOST},
};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{Manager, State, plugin::TauriPlugin};
use tokio::{net::TcpListener, sync::Mutex};

use self::{
    proxy::Proxy,
    sessions::{Key, Sessions},
    tls::Target,
};

/// Core's own limit on a device name.
const MAX_DEVICE_NAME: usize = 64;

pub fn init<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("cores")
        .setup(|app, _| {
            app.manage(Tunnels::default());
            app.manage(Arc::new(Sessions::default()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_connect,
            core_sign_in,
            core_sign_out,
            core_session_exists
        ])
        .build()
}

/// One open proxy: whose session it signs with, and the Core it reaches.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Tunnel {
    key: Key,
    address: String,
    fingerprint: String,
}

/// Open proxies and their local addresses.
#[derive(Default)]
struct Tunnels(Mutex<HashMap<Tunnel, (String, Arc<Proxy>)>>);

/// Why a call to a Core failed: its status and the error code and description it answered with,
/// in Core's own error shape. `status` is null when the Core did not answer.
#[derive(Debug, Serialize)]
struct CoreError {
    status: Option<u16>,
    error: String,
    description: String,
}

impl CoreError {
    fn unreachable(problem: impl std::fmt::Display) -> Self {
        Self {
            status: None,
            error: "unreachable".into(),
            description: format!("The Core did not answer: {problem}"),
        }
    }

    fn answered(status: StatusCode, body: &[u8]) -> Self {
        let body: Value = serde_json::from_slice(body).unwrap_or_default();
        let field = |name: &str, fallback: &str| {
            body[name].as_str().unwrap_or(fallback).to_owned()
        };
        Self {
            status: Some(status.as_u16()),
            error: field("error", "internal"),
            description: field("description", "The Core refused."),
        }
    }
}

/// Returns the local base URL (`http://127.0.0.1:<port>/<capability>`, see `proxy`) that reaches
/// the Core `core_id` at `address` (`https://host:port`), trusting only the certificate with this
/// SHA-256 `fingerprint`, and signing requests with `account`'s session there, when it has one.
#[tauri::command]
async fn core_connect(
    core_id: String,
    account: String,
    address: String,
    fingerprint: String,
    tunnels: State<'_, Tunnels>,
    sessions: State<'_, Arc<Sessions>>,
) -> Result<String, String> {
    let mut open = tunnels.0.lock().await;
    let tunnel = Tunnel {
        key: Key { account, core_id },
        address,
        fingerprint,
    };
    if let Some((local, _)) = open.get(&tunnel) {
        return Ok(local.clone());
    }
    let proxy = Arc::new(Proxy {
        target: Target::new(&tunnel.address, &tunnel.fingerprint)?,
        sessions: Arc::clone(&sessions),
        key: tunnel.key.clone(),
        capability: proxy::capability(),
    });
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    tokio::spawn(proxy::serve(listener, Arc::clone(&proxy)));
    let local = format!("http://127.0.0.1:{port}/{}", proxy.capability);
    open.insert(tunnel, (local.clone(), proxy));
    Ok(local)
}

/// Trade a `ticket` for a session on the Core at `address`, named after this computer, and keep
/// the token for `account`. Returns the session, never the token.
#[tauri::command]
async fn core_sign_in(
    core_id: String,
    account: String,
    address: String,
    fingerprint: String,
    ticket: String,
    sessions: State<'_, Arc<Sessions>>,
) -> Result<Value, CoreError> {
    let target =
        Target::new(&address, &fingerprint).map_err(|problem| CoreError {
            status: None,
            error: "invalid_request".into(),
            description: problem,
        })?;
    let body = json!({ "ticket": ticket, "device_name": device_name() });
    let request = Request::builder()
        .method(Method::POST)
        .uri("/sessions")
        .header(HOST, target.authority())
        .header(CONTENT_TYPE, "application/json")
        .body(Full::from(body.to_string()))
        .expect("a valid request");
    let (status, body) =
        target.call(request).await.map_err(CoreError::unreachable)?;
    if !status.is_success() {
        return Err(CoreError::answered(status, &body));
    }
    let mut signed: Value =
        serde_json::from_slice(&body).map_err(CoreError::unreachable)?;
    let Some(Value::String(token)) = signed.get_mut("token").map(Value::take)
    else {
        return Err(CoreError::unreachable("no session token in the answer"));
    };
    // The ticket was fetched for whoever was signed in then. A session for anyone but `account`,
    // as after an account switch mid sign-in, is ended and never kept.
    if signed["session"]["user"]["id"].as_str() != Some(account.as_str()) {
        end_session(&target, &token).await;
        return Err(CoreError {
            status: None,
            error: "invalid_request".into(),
            description: "The account changed while signing in. Try again."
                .into(),
        });
    }
    sessions.store(&Key { account, core_id }, token).await;
    Ok(signed["session"].take())
}

/// Sign `account` out of the Core: end the session there through the first of its open proxies
/// that answers, then forget the token either way.
#[tauri::command]
async fn core_sign_out(
    core_id: String,
    account: String,
    tunnels: State<'_, Tunnels>,
    sessions: State<'_, Arc<Sessions>>,
) -> Result<(), String> {
    let key = Key { account, core_id };
    if let Some(token) = sessions.token(&key).await {
        let proxies: Vec<Arc<Proxy>> = tunnels
            .0
            .lock()
            .await
            .iter()
            .filter(|(tunnel, _)| tunnel.key == key)
            .map(|(_, (_, proxy))| Arc::clone(proxy))
            .collect();
        for proxy in proxies {
            if end_session(&proxy.target, &token).await {
                break;
            }
        }
    }
    sessions.forget(&key).await;
    Ok(())
}

/// End the session `token` on the Core. False when the Core did not answer.
async fn end_session(target: &Target, token: &str) -> bool {
    let request = Request::builder()
        .method(Method::DELETE)
        .uri("/sessions/current")
        .header(HOST, target.authority())
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Full::<Bytes>::default())
        .expect("a valid request");
    match target.call(request).await {
        Ok(_) => true,
        Err(problem) => {
            tracing::debug!(%problem, "could not end a Core session there");
            false
        }
    }
}

/// Whether `account` holds a session for the Core. It may still have expired there.
#[tauri::command]
async fn core_session_exists(
    core_id: String,
    account: String,
    sessions: State<'_, Arc<Sessions>>,
) -> Result<bool, String> {
    Ok(sessions.token(&Key { account, core_id }).await.is_some())
}

/// This computer's name, as Core lists the session.
fn device_name() -> String {
    let name = whoami::fallible::hostname().unwrap_or_default();
    let name = name.trim();
    if name.is_empty() {
        "Amberite App".into()
    } else {
        name.chars().take(MAX_DEVICE_NAME).collect()
    }
}

#[cfg(test)]
mod tests;
