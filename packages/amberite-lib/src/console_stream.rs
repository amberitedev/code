//! WebSocket console streaming — Core `/ws/:id/console` → Tauri `console-line` events.

use crate::error::{AmberiteError, Result};
use futures_util::StreamExt;
use serde::Serialize;
use tauri::Emitter;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Payload emitted as the `console-line` Tauri event.
#[derive(Debug, Clone, Serialize)]
pub struct ConsoleLine {
    pub instance_id: String,
    pub line: String,
    pub timestamp: i64,
}

/// Start a WebSocket console stream for `instance_id`.
/// Spawns a background task that emits `console-line` events via `app_handle`.
/// Returns a `JoinHandle` so the stream can be stopped.
pub async fn start_console_stream<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    core_url: &str,
    instance_id: String,
) -> Result<JoinHandle<()>> {
    // Get a WS token from Core
    let client = reqwest::Client::new();
    let token_url = format!(
        "{}/instances/{}/ws-token",
        core_url.trim_end_matches('/'),
        instance_id
    );
    let token_resp = client
        .get(&token_url)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    let json: serde_json::Value = token_resp
        .json()
        .await
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    let token = json["token"].as_str().map(String::from).ok_or_else(|| {
        AmberiteError::Core("No token in ws-token response".into())
    })?;

    let ws_base = core_url
        .trim_end_matches('/')
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let ws_url =
        format!("{ws_base}/instances/{instance_id}/console?token={token}");

    let id = instance_id.clone();
    let handle = tokio::spawn(async move {
        if let Err(e) = stream_loop(app_handle, &ws_url, &id).await {
            tracing::warn!("Console stream error for {id}: {e}");
        }
    });

    Ok(handle)
}

async fn stream_loop<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    ws_url: &str,
    instance_id: &str,
) -> Result<()> {
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .map_err(|e| AmberiteError::WebSocket(e.to_string()))?;

    let (_, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(line)) => {
                let event = ConsoleLine {
                    instance_id: instance_id.to_string(),
                    line: line.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                };
                let _ = app.emit("console-line", event);
            }
            Ok(Message::Close(_)) => break,
            Err(e) => {
                tracing::warn!("WebSocket error: {e}");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
