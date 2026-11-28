//! HTTP and the `/events` socket over Core's local socket, which only `copal` and the owner may
//! open. A request that arrives this way is the owner's, with no token or ticket.

use axum::http::{HeaderValue, Request, header};
use bytes::Bytes;
use color_eyre::eyre::{Result, WrapErr, bail};
use contracts::{
    ApiError, Empty,
    core::{CheckHealth, CoreEvent},
};
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use tokio_tungstenite::WebSocketStream;

use crate::{
    access::local::{self, Stream},
    daemon::{config, install, paths::Paths, service},
};

/// Core answered with an error.
#[derive(Debug, thiserror::Error)]
#[error("{}", .0.description)]
pub struct Refused(pub ApiError);

/// Core could not be reached over the local socket.
#[derive(Debug, thiserror::Error)]
#[error("Core is not running, or this account may not use it")]
pub struct Unreachable;

/// Something is in the way, and there is one thing to do about it. A terminal shows the two
/// apart: what is wrong, then the next step where every command puts its next step.
#[derive(Debug, thiserror::Error)]
#[error("{what} {next}")]
pub struct Stuck {
    pub what: String,
    pub next: String,
}

/// Why Core does not answer, as far as this user can tell, and the one thing to do next.
pub fn why_unreachable() -> Stuck {
    let (what, next) = match service::state() {
        service::State::NotRegistered => (
            NOT_SET_UP,
            install::as_admin("Set it up with", "copal setup"),
        ),
        service::State::Stopped => {
            (NOT_RUNNING, "Find out why with: copal doctor".to_owned())
        }
        service::State::Running => {
            (NOT_ANSWERING, "Find out why with: copal doctor".to_owned())
        }
    };
    Stuck {
        what: what.into(),
        next,
    }
}

pub const NOT_SET_UP: &str = "Core is not set up on this computer yet.";
pub const NOT_RUNNING: &str =
    "Core is set up on this computer, but it is not running.";
pub const NOT_ANSWERING: &str =
    "Core's service runs, but it answers only the user who set it up.";

/// What to say when Core drops the connection unanswered. That is how it refuses an account
/// other than the owner's, root included, but a Core that stops mid-request looks the same, so
/// the message says what Core does rather than claiming a refusal. Under `sudo` the way out is
/// to run the command without it.
fn refused_message() -> String {
    let closed = "Core closed the connection without answering. It does that for every account but the one that set it up";
    match std::env::var("SUDO_USER") {
        Ok(user) if !user.is_empty() => format!(
            "{closed}, root included: run it again without sudo, as that account (was it {user}?)"
        ),
        _ => format!(
            "{closed}; if that is not this account, use that one. Otherwise see copal logs"
        ),
    }
}

/// One connection per request.
pub struct Client {
    paths: Paths,
}

impl Client {
    /// The Core on this machine's data folder, or `CORE_DATA_DIR`'s.
    pub fn new() -> Self {
        Self::at(Paths::new(config::data_dir()))
    }

    /// The Core on the data folder `paths`.
    pub fn at(paths: Paths) -> Self {
        Self { paths }
    }

    /// Whether Core answers on its local socket.
    pub async fn is_up(&self) -> bool {
        super::call::call::<CheckHealth>(self, &(), &(), Empty)
            .await
            .is_ok()
    }

    /// Send `request` and return the status, content type and body of a successful answer.
    pub(super) async fn send(
        &self,
        request: Request<Bytes>,
    ) -> Result<(u16, Option<String>, Bytes)> {
        let mut sender = hyper::client::conn::http1::handshake(TokioIo::new(
            self.connect().await?,
        ))
        .await
        .map(|(sender, connection)| {
            tokio::spawn(connection);
            sender
        })?;
        let path = request.uri().path().to_owned();
        let (mut parts, body) = request.into_parts();
        parts
            .headers
            .insert(header::HOST, HeaderValue::from_static("copal"));
        // Core drops a connection from an account other than `copal` and the owner without a
        // word, which reads here as a broken pipe or a closed connection.
        let response = sender
            .send_request(Request::from_parts(parts, Full::new(body)))
            .await
            .wrap_err_with(refused_message)?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.into_body().collect().await?.to_bytes();
        if status == axum::http::StatusCode::NOT_FOUND && body.is_empty() {
            bail!(
                "this Core does not know {path}; it may be older than this copal CLI"
            );
        }
        if !status.is_success() {
            match serde_json::from_slice::<ApiError>(&body) {
                Ok(error) => return Err(Refused(error).into()),
                Err(_) => bail!(
                    "Core answered {status}: {}",
                    String::from_utf8_lossy(&body).trim()
                ),
            }
        }
        Ok((status.as_u16(), content_type, body))
    }

    /// The `/events` socket.
    async fn events(&self) -> Result<WebSocketStream<Box<dyn Stream>>> {
        let (socket, _) = tokio_tungstenite::client_async(
            "ws://copal/events",
            self.connect().await?,
        )
        .await
        .wrap_err("Core refused the event socket")?;
        Ok(socket)
    }

    /// Run a long op: `start` sends its route with the op id, and this waits for its `OpEnded`. The
    /// socket opens first, so the end is never missed.
    pub async fn op<F, Fut>(&self, start: F) -> Result<()>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        use futures::StreamExt;
        use tokio_tungstenite::tungstenite::Message;

        let op = uuid::Uuid::new_v4().to_string();
        let mut socket = self.events().await?;
        start(op.clone()).await?;
        while let Some(message) = socket.next().await {
            let Message::Text(text) = message? else {
                continue;
            };
            // The engine's events read as no `CoreEvent`, and are skipped.
            if let Ok(CoreEvent::OpEnded { op: ended, error }) =
                serde_json::from_str(&text)
                && ended == op
            {
                return match error {
                    Some(error) => Err(Refused(error).into()),
                    None => Ok(()),
                };
            }
        }
        bail!("The event socket closed before the operation ended")
    }

    async fn connect(&self) -> Result<Box<dyn Stream>> {
        local::connect(&self.paths)
            .await
            .map_err(|_| Unreachable.into())
    }
}
