//! Paired Cores: a direct connection to each, and this device's session there.
//!
//! The WebView cannot talk to a Core itself. The App reaches each over Connect (see `tunnel` and
//! the `connect` crate): QUIC to the program that holds the certificate the App learned at
//! pairing, with no port forwarded. The App's Core proxy (see `proxy`) carries every request over
//! it and signs it with this device's session there. Requests, uploads, and console sockets all
//! go through it.
//!
//! A Core on this computer or its network is dialed at its own address. Any other answers only
//! devices the backend introduced, and the account session never leaves the WebView, so the
//! WebView carries the introduction: [`ConnectCore`] answers with what to introduce, and is
//! called again with where the backend says the Core is.
//!
//! A session comes from a ticket: the WebView gets one from the backend and hands it to the
//! sign-in route, which trades it with the Core for a session token and keeps the token in the OS
//! keychain (see `sessions`). No route returns a token.

pub mod proxy;
mod sessions;
mod tunnel;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{Router, extract::State};
use connect::{PeerId, on_own_network};
use contracts::{
    ApiError, Empty, Endpoint, ErrorCode, Json,
    backend::cores::Dialer,
    client::{CallError, Encode, Reply, request, response},
    core::{CoreSession, NewSession, SignIn, SignOut},
    server::{Mount, Req},
};
use http_body_util::Full;
use hyper::header::{HOST, HeaderValue};
use sharing_client::net::CoreAddress;

use self::{
    sessions::{Key, Sessions},
    tunnel::{Dialers, Tunnel},
};
use crate::local_api::endpoints::cores::{
    CheckCoreSession, ConnectCore, CoreRoute, CoreTarget, SignInToCore,
    SignOutOfCore,
};

/// Core's own limit on a device name.
const MAX_DEVICE_NAME: usize = 64;
/// How long a Core on this computer or its network may take to answer before the backend is
/// asked for an introduction.
const LOCAL_ANSWER: Duration = Duration::from_millis(2500);
/// How long an introduced Core may take to answer: both routers have to let the other side in.
const INTRODUCED_ANSWER: Duration = Duration::from_secs(10);

pub struct Cores {
    sessions: Sessions,
    /// `/core/<n>` on the proxy reaches `tunnels[n]`. Never shrinks, so a URL stays valid.
    tunnels: Mutex<Vec<Arc<Tunnel>>>,
    dialers: Arc<Dialers>,
    /// `http://127.0.0.1:<proxy port>/<secret>`.
    base: String,
}

impl Cores {
    pub fn new(base: String) -> Self {
        Self {
            sessions: Sessions::default(),
            tunnels: Mutex::default(),
            dialers: Arc::new(Dialers::new()),
            base,
        }
    }

    /// The tunnel to the Core `core` as `key`'s account, made on first use, and its number.
    fn tunnel_to(&self, key: Key, core: PeerId) -> (usize, Arc<Tunnel>) {
        let mut tunnels = self.tunnels.lock().expect("not poisoned");
        let n = tunnels
            .iter()
            .position(|tunnel| tunnel.key == key && tunnel.core == core)
            .unwrap_or_else(|| {
                tunnels.push(Arc::new(Tunnel::new(
                    key,
                    core,
                    Arc::clone(&self.dialers),
                )));
                tunnels.len() - 1
            });
        (n, Arc::clone(&tunnels[n]))
    }

    fn tunnel(&self, n: usize) -> Option<Arc<Tunnel>> {
        self.tunnels.lock().expect("not poisoned").get(n).cloned()
    }

    /// The account's tunnels to a Core, whichever certificate each was made for.
    fn tunnels_of(&self, key: &Key) -> Vec<Arc<Tunnel>> {
        self.tunnels
            .lock()
            .expect("not poisoned")
            .iter()
            .filter(|tunnel| tunnel.key == *key)
            .cloned()
            .collect()
    }

    /// Connect to the Core unless a connection is open: where the backend's introduction says
    /// it is, or with none, at its own address when this computer is on that network. A Core
    /// not reached with no introduction is answered with what to introduce.
    async fn connect(&self, target: CoreTarget) -> Result<CoreRoute, ApiError> {
        let core = target.fingerprint.parse().map_err(|problem| {
            ApiError::new(ErrorCode::InvalidInput, problem)
        })?;
        let key = Key {
            account: target.account,
            core_id: target.core_id,
        };
        let (n, tunnel) = self.tunnel_to(key, core);
        let base = format!("{}/core/{n}", self.base);
        let mut dialer = None;
        if tunnel.reached().is_none() {
            match target.introduced {
                Some(candidates) => {
                    tunnel.dial(&candidates, INTRODUCED_ANSWER).await;
                }
                None => {
                    let own = CoreAddress::parse(&target.address)
                        .ok()
                        .map(CoreAddress::socket)
                        .filter(|address| on_own_network(address.ip()));
                    let answered = match own {
                        Some(address) => {
                            tunnel.dial(&[address], LOCAL_ANSWER).await
                        }
                        None => false,
                    };
                    // A Core that listens on loopback is only ever on this computer, so
                    // there is nobody to be introduced to.
                    let here = own.is_some_and(|own| own.ip().is_loopback());
                    if !answered && !here {
                        dialer = Some(self.dialer().await?);
                    }
                }
            }
        }
        Ok(CoreRoute {
            base,
            reached: tunnel.reached(),
            dialer,
        })
    }

    /// This device as the backend introduces it to a Core on another network.
    async fn dialer(&self) -> Result<Dialer, ApiError> {
        let endpoint = self.dialers.toward(false).await.map_err(|error| {
            ApiError::new(
                ErrorCode::NetworkError,
                format!("This computer could not open a connection: {error}"),
            )
        })?;
        Ok(Dialer {
            id: endpoint.id().to_string(),
            candidates: endpoint.candidates(&[]).await,
        })
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
) -> Result<Json<CoreRoute>, ApiError> {
    cores.connect(body).await.map(Json)
}

async fn sign_in(
    State(cores): State<Arc<Cores>>,
    Req { body, .. }: Req<SignInToCore>,
) -> Result<Json<CoreSession>, ApiError> {
    let core = body
        .fingerprint
        .parse()
        .map_err(|problem| ApiError::new(ErrorCode::InvalidInput, problem))?;
    let key = Key {
        account: body.account.clone(),
        core_id: body.core_id,
    };
    let (_, tunnel) = cores.tunnel_to(key.clone(), core);
    let new_session = NewSession {
        ticket: body.ticket,
        device_name: device_name(),
    };
    let signed = call::<SignIn>(&tunnel, Json(new_session), None)
        .await
        .map_err(|error| match error {
            CallError::Api { error, .. } => error,
            other => unreachable(other),
        })?;
    // The ticket was fetched for whoever was signed in then. A session for anyone but `account`,
    // as after an account switch mid sign-in, is ended and never kept.
    if signed.session.user.id != body.account {
        end_session(&tunnel, &signed.token).await;
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "The account changed while signing in. Try again.",
        ));
    }
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
        for tunnel in cores.tunnels_of(&key) {
            if end_session(&tunnel, &token).await {
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
async fn end_session(tunnel: &Tunnel, token: &str) -> bool {
    match call::<SignOut>(tunnel, Empty, Some(token)).await {
        Err(CallError::Transport(problem)) => {
            tracing::debug!(%problem, "could not end a Core session there");
            false
        }
        _ => true,
    }
}

/// One call to a Core's own route over its tunnel.
async fn call<E: Endpoint<Params = (), Query = ()>>(
    tunnel: &Tunnel,
    body: <E::Body as Encode>::Input,
    bearer: Option<&str>,
) -> Result<<E::Response as Reply>::Output, CallError>
where
    E::Body: Encode,
    E::Response: Reply,
{
    let mut request = request::<E>("", &(), &(), body, bearer)?;
    request
        .headers_mut()
        .insert(HOST, HeaderValue::from_static(connect::tls::SERVER_NAME));
    let (status, answer) = tunnel
        .call(request.map(Full::new))
        .await
        .map_err(|problem| CallError::Transport(problem))?;
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
