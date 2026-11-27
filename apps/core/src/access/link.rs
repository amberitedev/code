//! The link: a WebSocket this Core holds to the backend while it is paired, so its owner's devices
//! can be introduced to it at any time (`contracts::backend::cores::LinkCore`).
//!
//! Nobody reaches a Core at an address they looked up. For each device that wants to connect, the
//! backend sends an introduction; Core lets that device's addresses in, sends toward them so the
//! home router does too, and answers with where it can be reached itself. The connection that
//! follows is direct (`network::connect`). Nothing else travels on the link, so it costs the
//! backend nothing while it waits.

use std::{convert::Infallible, time::Duration};

use bytes::Bytes;
use contracts::{
    Endpoint,
    backend::cores::{Introduced, Introduction, LinkCore},
};
use futures::{SinkExt, StreamExt};
use reqwest::{
    StatusCode,
    header::{CONNECTION, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, handshake::client::generate_key, protocol::Role},
};
use tracing::{debug, info};

use super::sync::Sync;

/// Ping this often, so nothing between here and the backend drops the link as idle. A link that
/// stays silent from one ping to the next is dead.
const PING: Duration = Duration::from_secs(30);

/// Wait before opening the link again after `failures` failures in a row: 5 seconds, doubling, at
/// most 5 minutes. A Core without a link cannot be reached from outside its own network.
fn backoff(failures: u32) -> Duration {
    Duration::from_secs((5u64 << failures.min(6)).min(300))
}

/// Hold the link for as long as the caller keeps polling, opening it again whenever it drops.
pub async fn hold(sync: &Sync, key: &str) -> Infallible {
    let mut failures = 0;
    loop {
        match open(sync, key).await {
            Ok(socket) => {
                failures = 0;
                info!("ready to be introduced to this Core's owner");
                serve(sync, socket).await;
            }
            Err(problem) => {
                failures += 1;
                debug!(%problem, "could not open the link to the backend");
            }
        }
        tokio::time::sleep(backoff(failures)).await;
    }
}

async fn open(
    sync: &Sync,
    key: &str,
) -> Result<WebSocketStream<reqwest::Upgraded>, String> {
    let response = sync
        .http
        .get(format!("{}{}", sync.backend_url, LinkCore::PATH))
        .bearer_auth(key)
        .header(CONNECTION, "upgrade")
        .header(UPGRADE, "websocket")
        .header(SEC_WEBSOCKET_VERSION, "13")
        .header(SEC_WEBSOCKET_KEY, generate_key())
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status() != StatusCode::SWITCHING_PROTOCOLS {
        return Err(format!("the backend answered {}", response.status()));
    }
    let upgraded = response
        .upgrade()
        .await
        .map_err(|error| error.to_string())?;
    Ok(WebSocketStream::from_raw_socket(upgraded, Role::Client, None).await)
}

/// Answer introductions until the link drops.
async fn serve(sync: &Sync, mut socket: WebSocketStream<reqwest::Upgraded>) {
    let mut ping = tokio::time::interval(PING);
    ping.tick().await;
    let mut silent = false;
    loop {
        tokio::select! {
            message = socket.next() => {
                silent = false;
                let text = match message {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                    _ => break,
                };
                let Ok(introduction) = serde_json::from_str::<Introduction>(&text) else {
                    debug!("the backend sent an introduction this Core cannot read");
                    continue;
                };
                let answer = Introduced {
                    candidates: sync.connect.introduce(&introduction.dialer.candidates).await,
                    request: introduction.request,
                };
                let answer = serde_json::to_string(&answer).expect("serializes");
                if socket.send(Message::Text(answer.into())).await.is_err() {
                    break;
                }
            }
            _ = ping.tick() => {
                if silent || socket.send(Message::Ping(Bytes::new())).await.is_err() {
                    break;
                }
                silent = true;
            }
        }
    }
    let _ = socket.close(None).await;
}
