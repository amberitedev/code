//! HTTP and the `/events` socket over Core's local socket, which only `copal` and the owner may
//! open. A request that arrives this way is the owner's, with no token or ticket.

use axum::http::{Method, Request, header};
use bytes::Bytes;
use color_eyre::eyre::{Result, WrapErr, bail};
use contracts::{
    ApiError,
    core::{ClientMessage, Topic},
};
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use serde::{Serialize, de::DeserializeOwned};
use tokio_tungstenite::WebSocketStream;

use crate::{
    access::local::{self, Stream},
    daemon::{config, paths::Paths},
};

/// Core answered with an error.
#[derive(Debug, thiserror::Error)]
#[error("{}", .0.description)]
pub struct Refused(pub ApiError);

/// Core could not be reached over the local socket.
#[derive(Debug, thiserror::Error)]
// `source` is printed as the cause by `{:#}`, so the message does not repeat it.
#[error(
    "Core is not running, or this account may not use it (could not open {path})"
)]
pub struct Unreachable {
    path: String,
    source: std::io::Error,
}

/// What to say when Core drops the connection unanswered. That is how it refuses an account
/// other than the owner's, root included, but a Core that stops mid-request looks the same, so
/// the message says what Core does rather than claiming a refusal. Under `sudo` the way out is
/// to run the command without it.
fn refused_message() -> String {
    let closed = "Core closed the connection without answering. It does that for every account but the one that installed Copal";
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

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.call(Method::GET, path, None::<&()>).await
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T> {
        self.call(Method::POST, path, Some(body)).await
    }

    /// Send a request and read the answer as `T`. An empty answer reads as `null`, so `()`
    /// takes it.
    pub async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<T> {
        let body = match body {
            Some(body) => Bytes::from(serde_json::to_vec(body)?),
            None => Bytes::new(),
        };
        let body = self.send(method, path, body).await?;
        let body: &[u8] = if body.is_empty() { b"null" } else { &body };
        serde_json::from_slice(body)
            .wrap_err("Core answered something this CLI cannot read")
    }

    /// Send a JSON request and return the body of a successful answer.
    pub(super) async fn send(
        &self,
        method: Method,
        path: &str,
        body: Bytes,
    ) -> Result<Bytes> {
        let mut sender = hyper::client::conn::http1::handshake(TokioIo::new(
            self.connect().await?,
        ))
        .await
        .map(|(sender, connection)| {
            tokio::spawn(connection);
            sender
        })?;
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "copal")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(body))?;
        // Core drops a connection from an account other than `copal` and the owner without a
        // word, which reads here as a broken pipe or a closed connection.
        let response = sender
            .send_request(request)
            .await
            .wrap_err_with(refused_message)?;
        let status = response.status();
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
        Ok(body)
    }

    /// The `/events` socket, subscribed to a server's console. It sends `console` events as JSON
    /// text, or `ended` when Core refused the subscription.
    pub async fn console(
        &self,
        server_id: &str,
    ) -> Result<WebSocketStream<Box<dyn Stream>>> {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let (mut socket, _) = tokio_tungstenite::client_async(
            "ws://copal/events",
            self.connect().await?,
        )
        .await
        .wrap_err("Core refused the event socket")?;
        let subscribe = ClientMessage::Subscribe {
            topic: Topic::Console {
                id: server_id.to_owned(),
            },
        };
        socket
            .send(Message::text(serde_json::to_string(&subscribe)?))
            .await?;
        Ok(socket)
    }

    async fn connect(&self) -> Result<Box<dyn Stream>> {
        local::connect(&self.paths).await.map_err(|source| {
            Unreachable {
                path: self.paths.local_socket().display().to_string(),
                source,
            }
            .into()
        })
    }
}
