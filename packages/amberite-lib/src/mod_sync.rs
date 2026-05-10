//! Mod sync — push modpack JSON to Core, subscribe to sync events.

use crate::error::{AmberiteError, Result};
use reqwest::Client;
use serde::Serialize;
use tauri::Emitter;

/// Payload emitted as `mod-sync-update` Tauri event.
#[derive(Debug, Clone, Serialize)]
pub struct ModSyncUpdate {
    pub group_id: String,
    pub instance_id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// Push a modpack manifest (JSON) to Core for a given instance.
/// The frontend builds the manifest from Theseus profile data.
pub async fn push_modpack_to_core(
    client: &Client,
    core_url: &Option<String>,
    instance_id: &str,
    modpack_json: serde_json::Value,
) -> Result<()> {
    let base = core_url
        .as_deref()
        .ok_or(AmberiteError::CoreNotConnected)?
        .trim_end_matches('/');
    let url = format!("{base}/instances/{instance_id}/modpack");
    let resp = client.post(&url).json(&modpack_json).send().await?;
    if !resp.status().is_success() {
        let msg = resp.text().await.unwrap_or_default();
        return Err(AmberiteError::Core(format!("Push modpack failed: {msg}")));
    }
    Ok(())
}

/// Subscribe to sync events for a Core instance (polls every 5 seconds).
/// Emits `mod-sync-update` Tauri events when changes are detected.
///
/// // AMBERITE TODO: Replace with real Supabase Realtime WS in V2
pub async fn subscribe_to_sync_events<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    client: &Client,
    core_url: &str,
    instance_id: String,
) -> Result<()> {
    let url = format!(
        "{}/instances/{instance_id}/sync-events",
        core_url.trim_end_matches('/')
    );
    let client = client.clone();

    tokio::spawn(async move {
        let mut last_seen_id: Option<String> = None;

        loop {
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(rows) = resp.json::<Vec<serde_json::Value>>().await {
                    if let Some(row) = rows.first() {
                        let row_id = row["id"].as_str().map(String::from);
                        if row_id != last_seen_id {
                            last_seen_id = row_id;
                            let event = ModSyncUpdate {
                                group_id: row["group_id"].as_str().unwrap_or("").to_string(),
                                instance_id: instance_id.clone(),
                                event_type: row["event_type"].as_str().unwrap_or("update").to_string(),
                                payload: row.clone(),
                            };
                            let _ = app_handle.emit("mod-sync-update", event);
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    Ok(())
}
