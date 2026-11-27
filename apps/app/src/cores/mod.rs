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
use contracts::{
    ApiError, Empty, Endpoint, ErrorCode, Json,
    client::{CallError, Encode, Reply, request, response},
    core::{CoreSession, NewSession, SignIn, SignOut},
    server::{Mount, Req},
};
use http_body_util::Full;
use hyper::header::HOST;

use self::{
    sessions::{Key, Sessions},
    tls::Target,
};
use crate::local_api::endpoints::cores::{
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
    Router::new()
        .endpoint::<ConnectCore, _, _>(connect)
        .endpoint::<SignInToCore, _, _>(sign_in)
        .endpoint::<SignOutOfCore, _, _>(sign_out)
        .endpoint::<CheckCoreSession, _, _>(session_exists)
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
    let new_session = NewSession {
        ticket: body.ticket,
        device_name: device_name(),
    };
    let signed = call::<SignIn>(&target, Json(new_session), None)
        .await
        .map_err(|error| match error {
            CallError::Api { error, .. } => error,
            other => unreachable(other),
        })?;
    // The ticket was fetched for whoever was signed in then. A session for anyone but `account`,
    // as after an account switch mid sign-in, is ended and never kept.
    if signed.session.user.id != body.account {
        end_session(&target, &signed.token).await;
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "The account changed while signing in. Try again.",
        ));
    }
    let key = Key {
        account: body.account,
        core_id: body.core_id,
    };
    cores.sessions.store(&key, signed.token).await;
    Ok(Json(signed.session))
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
    match call::<SignOut>(target, Empty, Some(token)).await {
        Err(CallError::Transport(problem)) => {
            tracing::debug!(%problem, "could not end a Core session there");
            false
        }
        _ => true,
    }
}

/// One call to a Core's own route over the tunnel pinned to it.
async fn call<E: Endpoint<Params = (), Query = ()>>(
    target: &Target,
    body: <E::Body as Encode>::Input,
    bearer: Option<&str>,
) -> Result<<E::Response as Reply>::Output, CallError>
where
    E::Body: Encode,
    E::Response: Reply,
{
    let mut request = request::<E>("", &(), &(), body, bearer)?;
    let authority = target
        .authority()
        .parse()
        .expect("an authority is a header value");
    request.headers_mut().insert(HOST, authority);
    let (status, answer) = target
        .call(request.map(Full::new))
        .await
        .map_err(CallError::Transport)?;
    response::<E>(status.as_u16(), None, answer)
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
