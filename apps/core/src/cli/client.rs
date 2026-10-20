//! HTTP and the console socket over Core's local socket, which only `copal` and the owner may
//! open. A request that arrives this way is the owner's, with no token or ticket.

use axum::http::{Method, Request, header};
use bytes::Bytes;
use color_eyre::eyre::{Result, WrapErr, bail};
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use serde::{Serialize, de::DeserializeOwned};
use tokio_tungstenite::WebSocketStream;

use crate::{
    access::local::{self, Stream},
    api::error::ApiError,
    daemon::{config, paths::Paths},
};

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
        let response = sender.send_request(request).await?;
        let status = response.status();
        let body = response.into_body().collect().await?.to_bytes();
        if status == axum::http::StatusCode::NOT_FOUND && body.is_empty() {
            bail!(
                "this Core does not know {path}; it may be older than this copal CLI"
            );
        }
        if !status.is_success() {
            match serde_json::from_slice::<ApiError>(&body) {
                Ok(error) => bail!("{}", error.description),
                Err(_) => bail!(
                    "Core answered {status}: {}",
                    String::from_utf8_lossy(&body).trim()
                ),
            }
        }
        let body: &[u8] = if body.is_empty() { b"null" } else { &body };
        serde_json::from_slice(body)
            .wrap_err("Core answered something this CLI cannot read")
    }

    /// A server's console socket, past its greeting. It sends `log` events as JSON text and
    /// takes `command` messages.
    pub async fn console(
        &self,
        server_id: &str,
    ) -> Result<WebSocketStream<Box<dyn Stream>>> {
        use futures::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;

        let (mut socket, _) = tokio_tungstenite::client_async(
            format!("ws://copal/servers/{server_id}/ws"),
            self.connect().await?,
        )
        .await
        .wrap_err("Core refused the console")?;
        // The hosting API's greeting. The local socket is the owner, so the token is not read.
        socket
            .send(Message::text(r#"{"event":"auth","jwt":""}"#))
            .await?;
        match socket.next().await {
            Some(Ok(Message::Text(text))) if text.contains("auth-ok") => {
                Ok(socket)
            }
            _ => bail!("Core closed the console"),
        }
    }

    async fn connect(&self) -> Result<Box<dyn Stream>> {
        local::connect(&self.paths).await.wrap_err_with(|| {
            format!(
                "Core is not running, or this account may not use it (could not open {})",
                self.paths.local_socket().display()
            )
        })
    }
}
