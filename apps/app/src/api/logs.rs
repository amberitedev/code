use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::logs::LogType;
use engine::logs::{self, CensoredString, LatestLogCursor, Logs};

/*
A log is a struct containing the filename string, stdout, and stderr, as follows:

pub struct Logs {
    pub filename:  String,
    pub stdout: String,
    pub stderr: String,
}
*/

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("logs")
        .js_init_script(include_str!("webview-logs.js"))
        .invoke_handler(tauri::generate_handler![
            logs_get_logs,
            logs_get_logs_by_filename,
            logs_get_output_by_filename,
            logs_delete_logs,
            logs_delete_logs_by_filename,
            logs_get_latest_log_cursor,
            logs_get_live_log_buffer,
            logs_clear_live_log_buffer,
            logs_log_webview,
        ])
        .build()
}

#[tauri::command]
pub fn logs_log_webview<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    level: &str,
    message: &str,
) {
    let label = webview.label();
    match level {
        "trace" => {
            tracing::trace!(target: "webview", webview = label, "{message}")
        }
        "debug" => {
            tracing::debug!(target: "webview", webview = label, "{message}")
        }
        "warn" => {
            tracing::warn!(target: "webview", webview = label, "{message}")
        }
        "error" => {
            tracing::error!(target: "webview", webview = label, "{message}")
        }
        _ => tracing::info!(target: "webview", webview = label, "{message}"),
    }
}

/// Get all logs for an instance, sorted by filename.
#[tauri::command]
pub async fn logs_get_logs(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    clear_contents: Option<bool>,
) -> Result<Vec<Logs>> {
    let state = handle.get().await;
    let val = logs::get_logs(&state, instance_id, clear_contents).await?;

    Ok(val)
}

/// Get a log struct for an instance by filename.
#[tauri::command]
pub async fn logs_get_logs_by_filename(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> Result<Logs> {
    let state = handle.get().await;
    Ok(
        logs::get_logs_by_filename(&state, instance_id, log_type, filename)
            .await?,
    )
}

/// Get the output for an instance by filename.
#[tauri::command]
pub async fn logs_get_output_by_filename(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> Result<CensoredString> {
    let state = handle.get().await;
    Ok(
        logs::get_output_by_filename(&state, instance_id, log_type, &filename)
            .await?,
    )
}

/// Delete all logs for an instance.
#[tauri::command]
pub async fn logs_delete_logs(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    Ok(logs::delete_logs(&state, instance_id).await?)
}

/// Delete a log for an instance by filename.
#[tauri::command]
pub async fn logs_delete_logs_by_filename(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> Result<()> {
    let state = handle.get().await;
    Ok(
        logs::delete_logs_by_filename(&state, instance_id, log_type, &filename)
            .await?,
    )
}

/// Get live log from a cursor
#[tauri::command]
pub async fn logs_get_latest_log_cursor(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    cursor: u64, // 0 to start at beginning of file
) -> Result<LatestLogCursor> {
    let state = handle.get().await;
    Ok(logs::get_latest_log_cursor(&state, instance_id, cursor).await?)
}

/// Get all buffered live log lines for an instance.
#[tauri::command]
pub async fn logs_get_live_log_buffer(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<CensoredString> {
    let state = handle.get().await;
    Ok(logs::get_live_log_buffer(&state, instance_id).await?)
}

/// Clear the live log buffer for an instance.
#[tauri::command]
pub async fn logs_clear_live_log_buffer(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    Ok(logs::clear_live_log_buffer(&state, instance_id).await?)
}
