//! Paired Cores: pinned connections, and this device's session on each.
//!
//! A Core serves HTTPS with a certificate it made itself, so the WebView cannot talk to it
//! directly. The App's Core proxy (see `proxy`) carries every request to a Core over TLS pinned to
//! the fingerprint the App learned at pairing, and signs it with this device's session there.
//! Requests, uploads, and console sockets all go through it.
//!
//! A session comes from a ticket: the WebView gets one from the backend and hands it to the
//! sign-in route, which trades it with the Core for a session token and keeps the token in the OS
//! keychain (see `sessions`). No route returns a token.

pub mod proxy;
mod sessions;
mod tls;

use std::sync::{Arc, Mutex};

use axum::{Router, extract::State};
use bytes::Bytes;
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    core::CoreSession,
    server::{Req, route},
};
use http_body_util::Full;
use hyper::{
    Method, Request,
    header::{AUTHORIZATION, CONTENT_TYPE, HOST},
};
use serde_json::{Value, json};

use self::{
    sessions::{Key, Sessions},
    tls::Target,
};
use crate::server::endpoints::cores::{
    CheckCoreSession, ConnectCore, SignInToCore, SignOutOfCore,
};

/// Core's own limit on a device name.
const MAX_DEVICE_NAME: usize = 64;

pub struct Cores {
    sessions: Sessions,
    /// `/core/<n>` on the proxy reaches `tunnels[n]`. Never shrinks, so a URL stays valid.
    tunnels: Mutex<Vec<Arc<Tunnel>>>,
    /// `http://127.0.0.1:<proxy port>/<secret>`.
    base: String,
}

/// A Core as one account reaches it.
struct Tunnel {
    key: Key,
    address: String,
    fingerprint: String,
    target: Target,
}

impl Cores {
    pub fn new(base: String) -> Self {
        Self {
            sessions: Sessions::default(),
            tunnels: Mutex::default(),
            base,
        }
    }

    /// The proxy URL that reaches the Core at `address` as `key`'s account.
    fn connect(
        &self,
        key: Key,
        address: String,
        fingerprint: String,
    ) -> Result<String, ApiError> {
        let mut tunnels = self.tunnels.lock().expect("not poisoned");
        let n = match tunnels.iter().position(|tunnel| {
            tunnel.key == key
                && tunnel.address == address
                && tunnel.fingerprint == fingerprint
        }) {
            Some(n) => n,
            None => {
                let target =
                    Target::new(&address, &fingerprint).map_err(|problem| {
                        ApiError::new(ErrorCode::InvalidInput, problem)
                    })?;
                tunnels.push(Arc::new(Tunnel {
                    key,
                    address,
                    fingerprint,
                    target,
                }));
                tunnels.len() - 1
            }
        };
        Ok(format!("{}/core/{n}", self.base))
    }

    fn tunnel(&self, n: usize) -> Option<Arc<Tunnel>> {
        self.tunnels.lock().expect("not poisoned").get(n).cloned()
    }
}

/// The `/app/cores` routes on the local server.
pub fn routes(cores: Arc<Cores>) -> Router {
    let (connect_path, connect) = route::<ConnectCore, _, _, _>(connect);
    let (sign_in_path, sign_in) = route::<SignInToCore, _, _, _>(sign_in);
    let (sign_out_path, sign_out) = route::<SignOutOfCore, _, _, _>(sign_out);
    let (exists_path, exists) =
        route::<CheckCoreSession, _, _, _>(session_exists);
    Router::new()
        .route(connect_path, connect)
        .route(sign_in_path, sign_in)
        .route(sign_out_path, sign_out)
        .route(exists_path, exists)
        .with_state(cores)
}

async fn connect(
    State(cores): State<Arc<Cores>>,
    Req { body, .. }: Req<ConnectCore>,
) -> Result<Json<String>, ApiError> {
    let key = Key {
        account: body.account,
        core_id: body.core_id,
    };
    cores.connect(key, body.address, body.fingerprint).map(Json)
}

async fn sign_in(
    State(cores): State<Arc<Cores>>,
    Req { body, .. }: Req<SignInToCore>,
) -> Result<Json<CoreSession>, ApiError> {
    let target = Target::new(&body.address, &body.fingerprint)
        .map_err(|problem| ApiError::new(ErrorCode::InvalidInput, problem))?;
    let request_body =
        json!({ "ticket": body.ticket, "device_name": device_name() });
    let request = Request::builder()
        .method(Method::POST)
        .uri("/sessions")
        .header(HOST, target.authority())
        .header(CONTENT_TYPE, "application/json")
        .body(Full::from(request_body.to_string()))
        .expect("a valid request");
    let (status, answer) = target.call(request).await.map_err(unreachable)?;
    if !status.is_success() {
        return Err(serde_json::from_slice(&answer).unwrap_or_else(|_| {
            ApiError::new(ErrorCode::InternalError, "The Core refused.")
        }));
    }
    let mut signed: Value =
        serde_json::from_slice(&answer).map_err(unreachable)?;
    let Some(Value::String(token)) = signed.get_mut("token").map(Value::take)
    else {
        return Err(unreachable("no session token in the answer"));
    };
    // The ticket was fetched for whoever was signed in then. A session for anyone but `account`,
    // as after an account switch mid sign-in, is ended and never kept.
    if signed["session"]["user"]["id"].as_str() != Some(body.account.as_str()) {
        end_session(&target, &token).await;
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "The account changed while signing in. Try again.",
        ));
    }
    let key = Key {
        account: body.account,
        core_id: body.core_id,
    };
    cores.sessions.store(&key, token).await;
    serde_json::from_value(signed["session"].take())
        .map(Json)
        .map_err(unreachable)
}

/// Ends the session there through the first of the account's tunnels to the Core that answers.
async fn sign_out(
    State(cores): State<Arc<Cores>>,
    Req { query, .. }: Req<SignOutOfCore>,
) -> Empty {
    let key = Key {
        account: query.account,
        core_id: query.core_id,
    };
    if let Some(token) = cores.sessions.token(&key).await {
        let tunnels: Vec<Arc<Tunnel>> = cores
            .tunnels
            .lock()
            .expect("not poisoned")
            .iter()
            .filter(|tunnel| tunnel.key == key)
            .cloned()
            .collect();
        for tunnel in tunnels {
            if end_session(&tunnel.target, &token).await {
                break;
            }
        }
    }
    cores.sessions.forget(&key).await;
    Empty
}

async fn session_exists(
    State(cores): State<Arc<Cores>>,
    Req { query, .. }: Req<CheckCoreSession>,
) -> Json<bool> {
    let key = Key {
        account: query.account,
        core_id: query.core_id,
    };
    Json(cores.sessions.token(&key).await.is_some())
}

fn unreachable(problem: impl std::fmt::Display) -> ApiError {
    ApiError::new(
        ErrorCode::NetworkError,
        format!("The Core did not answer: {problem}"),
    )
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

/// This computer's name, as Core lists the session.
fn device_name() -> String {
    let name = whoami::hostname().unwrap_or_default();
    let name = name.trim();
    if name.is_empty() {
        "Amberite App".into()
    } else {
        name.chars().take(MAX_DEVICE_NAME).collect()
    }
}

#[cfg(test)]
mod tests;
