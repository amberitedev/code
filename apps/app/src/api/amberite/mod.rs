//! Amberite plugin — `plugin:amberite|*` namespace.
//!
//! All Amberite-specific commands live here.
//!
//! Debug: `invoke('plugin:amberite|ping')` → "amberite-lib: pong"

use amberite_lib::core_instances::{
    get_instance, restart_instance, send_command, start_instance, stop_instance, InstanceDetail,
};
use amberite_lib::error::AmberiteError;
use amberite_lib::settings::AppSettings;
use reqwest::Client;
use tauri::plugin::TauriPlugin;
use tauri::Runtime;

use crate::api::{Result, TheseusSerializableError};

const DEV_CORE_URL: &str = "http://localhost:16662";

async fn resolve_core_url() -> String {
    AppSettings::load()
        .await
        .ok()
        .and_then(|s| s.core_url)
        .unwrap_or_else(|| DEV_CORE_URL.to_string())
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tracing::info!("Amberite plugin initializing — amberite-lib loaded");
    tauri::plugin::Builder::<R>::new("amberite")
        .invoke_handler(tauri::generate_handler![
            ping,
            core_get_instance,
            core_start,
            core_stop,
            core_restart,
            core_send_command,
            core_issue_ws_token,
            core_get_url,
        ])
        .build()
}

/// Debug command — confirms the amberite plugin is reachable from the frontend.
///
/// Frontend: `invoke('plugin:amberite|ping')` → `"amberite-lib: pong"`
#[tauri::command]
async fn ping() -> String {
    tracing::info!("amberite ping invoked");
    "amberite-lib: pong".to_string()
}

/// Fetch full details for a Core instance by ID.
#[tauri::command]
async fn core_get_instance(id: String) -> Result<InstanceDetail> {
    let url = resolve_core_url().await;
    get_instance(&Client::new(), &Some(url), &id)
        .await
        .map_err(TheseusSerializableError::Amberite)
}

/// Start a Core server instance.
#[tauri::command]
async fn core_start(id: String) -> Result<()> {
    let url = resolve_core_url().await;
    start_instance(&Client::new(), &Some(url), &id)
        .await
        .map_err(TheseusSerializableError::Amberite)
}

/// Stop a Core server instance gracefully.
#[tauri::command]
async fn core_stop(id: String) -> Result<()> {
    let url = resolve_core_url().await;
    stop_instance(&Client::new(), &Some(url), &id)
        .await
        .map_err(TheseusSerializableError::Amberite)
}

/// Restart a Core server instance.
#[tauri::command]
async fn core_restart(id: String) -> Result<()> {
    let url = resolve_core_url().await;
    restart_instance(&Client::new(), &Some(url), &id)
        .await
        .map_err(TheseusSerializableError::Amberite)
}

/// Send a console command to a running Core instance.
#[tauri::command]
async fn core_send_command(id: String, command: String) -> Result<()> {
    let url = resolve_core_url().await;
    send_command(&Client::new(), &Some(url), &id, &command)
        .await
        .map_err(TheseusSerializableError::Amberite)
}

/// Issue a one-time WebSocket ticket from Core (POST /ws-token).
/// In dev mode, Core bypasses auth. In production, Core requires Authorization header.
// TODO: AMBERITE - pass supabase_token from AppSettings for production auth
#[tauri::command]
async fn core_issue_ws_token() -> Result<String> {
    let base = resolve_core_url().await;
    let res: serde_json::Value = Client::new()
        .post(format!("{base}/ws-token"))
        .send()
        .await
        .map_err(|e| AmberiteError::Core(e.to_string()))
        .map_err(TheseusSerializableError::Amberite)?
        .error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))
        .map_err(TheseusSerializableError::Amberite)?
        .json()
        .await
        .map_err(|e| AmberiteError::Core(e.to_string()))
        .map_err(TheseusSerializableError::Amberite)?;
    res["ticket"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| {
            TheseusSerializableError::Amberite(AmberiteError::Core("missing ticket".into()))
        })
}

/// Get the effective Core base HTTP URL (used by the frontend to build WebSocket URLs).
#[tauri::command]
async fn core_get_url() -> String {
    resolve_core_url().await
}
