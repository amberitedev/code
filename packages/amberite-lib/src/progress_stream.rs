//! SSE progress streaming — Core `/instances/:id/progress` → Tauri `instance-progress` events.

use crate::error::{AmberiteError, Result};
use futures_util::StreamExt;
use serde::Serialize;
use tauri::Emitter;

/// Payload emitted as the `instance-progress` Tauri event.
#[derive(Debug, Clone, Serialize)]
pub struct InstanceProgress {
    pub instance_id: String,
    pub stage: String,
    pub percent: f32,
}

/// Subscribe to the SSE progress stream for an instance creation/operation.
/// Streams until the server closes the connection, emitting `instance-progress` events.
pub async fn subscribe<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    core_url: &str,
    instance_id: &str,
) -> Result<()> {
    let url = format!(
        "{}/instances/{instance_id}/progress",
        core_url.trim_end_matches('/')
    );

    let resp = reqwest::Client::new()
        .get(&url)
        .header("Accept", "text/event-stream")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(AmberiteError::Core(format!(
            "Progress stream HTTP {}",
            resp.status()
        )));
    }

    let id = instance_id.to_string();
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buf.find('\n') {
            let line = buf[..newline_pos].trim_end_matches('\r').to_string();
            buf = buf[newline_pos + 1..].to_string();

            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if let Ok(event) = serde_json::from_str::<ProgressEvent>(data) {
                    let payload = InstanceProgress {
                        instance_id: id.clone(),
                        stage: event.stage,
                        percent: event.percent,
                    };
                    let _ = app_handle.emit("instance-progress", payload);
                    if event.percent >= 100.0 {
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct ProgressEvent {
    stage: String,
    percent: f32,
}
