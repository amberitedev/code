//! Amberite plugin — `plugin:amberite|*` namespace.
//!
//! Reduced to OS-level invokes: session token keychain access, Core health, URL resolve.
//! All API calls are routed through `@tauri-apps/plugin-http` (tauriFetch)
//! in TypeScript; no per-endpoint Rust proxies remain.
//!
//! Debug: `invoke('plugin:amberite|ping')` → "amberite-lib: pong"

use amberite_lib::core_launcher::{
	get_local_setup_secret as read_local_setup_secret,
	is_core_running as check_core_running,
};
use amberite_lib::session;
use amberite_lib::settings::AppSettings;
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
			get_current_jwt,
			set_current_jwt,
			clear_current_jwt,
			get_local_setup_secret,
			is_core_running,
			core_get_url,
		])
		.build()
}

/// Debug command — confirms the amberite plugin is reachable from the frontend.
#[tauri::command]
async fn ping() -> String {
	tracing::info!("amberite ping invoked");
	"amberite-lib: pong".to_string()
}

/// Read the current Amberite session JWT from the OS keychain.
#[tauri::command]
async fn get_current_jwt() -> Result<Option<String>> {
	session::get_current_jwt().map_err(TheseusSerializableError::Amberite)
}

/// Persist the current Amberite session JWT in the OS keychain.
#[tauri::command]
async fn set_current_jwt(jwt: String) -> Result<()> {
	session::set_current_jwt(jwt).map_err(TheseusSerializableError::Amberite)
}

/// Clear the current Amberite session JWT from the OS keychain.
#[tauri::command]
async fn clear_current_jwt() -> Result<()> {
	session::clear_current_jwt().map_err(TheseusSerializableError::Amberite)
}

/// Read the local one-time setup secret for an app-launched Core.
#[tauri::command]
async fn get_local_setup_secret() -> Result<Option<String>> {
	read_local_setup_secret()
		.await
		.map_err(TheseusSerializableError::Amberite)
}

/// Check whether the local Core process is responding on its HTTP health endpoint.
#[tauri::command]
async fn is_core_running() -> bool {
	let url = resolve_core_url().await;
	check_core_running(Some(&url)).await
}

/// Get the effective Core base HTTP URL (used by the frontend to build WebSocket URLs).
#[tauri::command]
async fn core_get_url() -> String {
	resolve_core_url().await
}
