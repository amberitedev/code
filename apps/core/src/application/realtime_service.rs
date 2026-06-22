use std::{sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::warn;

use crate::application::state::AppState;

#[derive(Deserialize)]
struct SessionResponse {
    ticket: String,
}

/// Maintains the Core's ephemeral presence connection without any polling.
pub async fn run_realtime_presence(state: Arc<AppState>) {
    let Some((endpoint, credential)) = state.realtime_config().await else {
        return;
    };
    let mut attempt = 0_u32;
    loop {
        match connect(state.clone(), &endpoint, &credential).await {
            Ok(()) => attempt = 0,
            Err(error) => warn!(%error, "Core realtime connection ended"),
        }
        let delay = Duration::from_millis(
            (500_u64.saturating_mul(2_u64.pow(attempt))).min(30_000),
        );
        attempt = attempt.saturating_add(1);
        tokio::time::sleep(delay).await;
    }
}

async fn connect(
    state: Arc<AppState>,
    endpoint: &str,
    credential: &str,
) -> Result<(), String> {
    let base = endpoint.trim_end_matches('/');
    let response = state
        .http
        .post(format!("{base}/v1/core-sessions"))
        .json(&json!({ "coreId": state.core_id, "credential": credential }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "realtime session endpoint returned {}",
            response.status()
        ));
    }
    let session: SessionResponse =
        response.json().await.map_err(|error| error.to_string())?;
    let scheme = if base.starts_with("https://") {
        "wss://"
    } else {
        "ws://"
    };
    let host = base
        .split_once("://")
        .map(|(_, value)| value)
        .unwrap_or(base);
    let (mut socket, _) = connect_async(format!(
        "{scheme}{host}/v1/connect?ticket={}",
        session.ticket
    ))
    .await
    .map_err(|error| error.to_string())?;
    socket.send(Message::Text(json!({ "type": "core.health", "health": "healthy", "diagnostic": "none" }).to_string().into())).await.map_err(|error| error.to_string())?;
    while let Some(message) = socket.next().await {
        match message.map_err(|error| error.to_string())? {
            Message::Close(_) => return Ok(()),
            Message::Ping(payload) => socket
                .send(Message::Pong(payload))
                .await
                .map_err(|error| error.to_string())?,
            _ => {}
        }
    }
    Ok(())
}
