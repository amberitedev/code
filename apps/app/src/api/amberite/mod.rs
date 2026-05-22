//! Amberite plugin — `plugin:amberite|*` namespace.
//!
//! Reduced to OS-level invokes: session token keychain access, Core health, URL resolve.
//! All API calls are routed through `@tauri-apps/plugin-http` (tauriFetch)
//! in TypeScript; no per-endpoint Rust proxies remain.
//!
//! Debug: `invoke('plugin:amberite|ping')` → "amberite-lib: pong"

use amberite_lib::core_launcher::{
    get_local_setup_secret as read_local_setup_secret, install_core_from_url,
    is_core_installed as check_core_installed,
    is_core_running as check_core_running, start_managed_core,
    stop_managed_core,
};
use amberite_lib::permissions::{list_permission_presets, PermissionPreset};
use amberite_lib::session;
use amberite_lib::settings::AppSettings;
use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::oneshot;

use crate::api::{oauth_utils, Result, TheseusSerializableError};

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
            core_get_settings,
            core_set_settings,
            core_list_permission_presets,
            get_current_jwt,
            set_current_jwt,
            clear_current_jwt,
            convex_login,
            convex_refresh_session,
            convex_logout,
            get_local_setup_secret,
            is_core_running,
            core_get_url,
            core_is_installed,
            core_install,
            core_start,
            core_stop,
        ])
        .build()
}

#[derive(Debug, Deserialize)]
struct ConvexActionResponse<T> {
    status: String,
    value: Option<T>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConvexSignInResponse {
    redirect: Option<String>,
    verifier: Option<String>,
    tokens: Option<ConvexTokens>,
}

#[derive(Debug, Deserialize)]
struct ConvexTokens {
    token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
}

fn amberite_error(message: impl Into<String>) -> TheseusSerializableError {
    TheseusSerializableError::Amberite(
        amberite_lib::error::AmberiteError::Auth(message.into()),
    )
}

fn decode_query_value(value: &str) -> String {
    urlencoding::decode(value)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| value.to_string())
}

fn jwt_expired(jwt: &str) -> bool {
    let Some(payload) = jwt.split('.').nth(1) else {
        return true;
    };
    let Ok(bytes) =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload)
    else {
        return true;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return true;
    };
    value
        .get("exp")
        .and_then(|exp| exp.as_i64())
        .is_none_or(|exp| chrono::Utc::now().timestamp() + 60 >= exp)
}

async fn convex_sign_in_action(
    convex_url: &str,
    body: serde_json::Value,
    bearer: Option<&str>,
) -> Result<ConvexSignInResponse> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| {
            amberite_error(format!("Convex auth client failed: {e}"))
        })?;
    let mut request = client
        .post(format!("{}/api/action", convex_url.trim_end_matches('/')))
        .header("Content-Type", "application/json")
        .json(&json!({
            "path": "auth:signIn",
            "format": "json",
            "args": body,
        }));
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|e| {
        amberite_error(format!("Convex auth request failed: {e}"))
    })?;
    let status = response.status();
    let body = response
        .json::<ConvexActionResponse<ConvexSignInResponse>>()
        .await
        .map_err(|e| {
            amberite_error(format!("Convex auth response was invalid: {e}"))
        })?;
    if !status.is_success() || body.status != "success" {
        return Err(amberite_error(
            body.error_message
                .unwrap_or_else(|| format!("Convex auth failed: {status}")),
        ));
    }
    body.value
        .ok_or_else(|| amberite_error("Convex auth returned no value"))
}

async fn store_convex_tokens(tokens: ConvexTokens) -> Result<String> {
    let token = tokens.token;
    session::set_current_tokens(token.clone(), tokens.refresh_token)
        .map_err(TheseusSerializableError::Amberite)?;
    Ok(token)
}

async fn refresh_convex_session(convex_url: &str) -> Result<Option<String>> {
    let Some(refresh_token) = session::get_refresh_token()
        .map_err(TheseusSerializableError::Amberite)?
    else {
        return Ok(None);
    };
    let result = convex_sign_in_action(
        convex_url,
        json!({ "refreshToken": refresh_token }),
        None,
    )
    .await?;
    match result.tokens {
        Some(tokens) => store_convex_tokens(tokens).await.map(Some),
        None => {
            session::clear_current_session()
                .map_err(TheseusSerializableError::Amberite)?;
            Ok(None)
        }
    }
}

#[tauri::command]
async fn core_get_settings() -> Result<AppSettings> {
    AppSettings::load()
        .await
        .map_err(TheseusSerializableError::Amberite)
}

#[tauri::command]
async fn core_set_settings(
    core_url: Option<String>,
    display_name: Option<String>,
    auto_launch_core: bool,
) -> Result<()> {
    AppSettings {
        core_url,
        display_name,
        auto_launch_core,
    }
    .save()
    .await
    .map_err(TheseusSerializableError::Amberite)
}

#[tauri::command]
async fn core_list_permission_presets() -> Result<Vec<PermissionPreset>> {
    list_permission_presets()
        .await
        .map_err(TheseusSerializableError::Amberite)
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
    session::clear_current_session().map_err(TheseusSerializableError::Amberite)
}

#[tauri::command]
async fn convex_refresh_session(convex_url: String) -> Result<Option<String>> {
    if let Some(jwt) = session::get_current_jwt()
        .map_err(TheseusSerializableError::Amberite)?
    {
        if !jwt_expired(&jwt) {
            return Ok(Some(jwt));
        }
    }
    refresh_convex_session(&convex_url).await
}

#[tauri::command]
async fn convex_logout() -> Result<()> {
    session::clear_current_session().map_err(TheseusSerializableError::Amberite)
}

#[tauri::command]
async fn convex_login<R: Runtime>(
    app: tauri::AppHandle<R>,
    convex_url: String,
) -> Result<String> {
    if let Some(jwt) = convex_refresh_session(convex_url.clone()).await? {
        return Ok(jwt);
    }

    let (auth_code_recv_socket_tx, auth_code_recv_socket) = oneshot::channel();
    let auth_code = tokio::spawn(oauth_utils::auth_code_reply::listen(
        auth_code_recv_socket_tx,
    ));

    let auth_code_recv_socket = auth_code_recv_socket.await.unwrap()?;
    let redirect_to = format!(
        "http://{}:{}",
        if auth_code_recv_socket.is_ipv4() {
            "127.0.0.1"
        } else {
            "[::1]"
        },
        auth_code_recv_socket.port()
    );

    let sign_in = convex_sign_in_action(
        &convex_url,
        json!({
            "provider": "microsoft-entra-id",
            "params": { "redirectTo": redirect_to },
        }),
        None,
    )
    .await?;
    let redirect = sign_in.redirect.ok_or_else(|| {
        amberite_error("Convex auth did not return a redirect")
    })?;
    let verifier = sign_in.verifier.ok_or_else(|| {
        amberite_error("Convex auth did not return a verifier")
    })?;

    app.opener().open_url(redirect, None::<&str>).map_err(|e| {
        amberite_error(format!("Failed to open Convex auth URL: {e}"))
    })?;

    let Some(code) = auth_code.await.unwrap()? else {
        return Err(amberite_error("Convex login canceled"));
    };
    let code = decode_query_value(&code);
    let result = convex_sign_in_action(
        &convex_url,
        json!({ "params": { "code": code }, "verifier": verifier }),
        None,
    )
    .await?;
    let tokens = result
        .tokens
        .ok_or_else(|| amberite_error("Convex auth did not return tokens"))?;

    if let Some(main_window) = app.get_window("main") {
        main_window.set_focus().ok();
    }

    store_convex_tokens(tokens).await
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

#[tauri::command]
async fn core_is_installed() -> bool {
    check_core_installed()
}

#[tauri::command]
async fn core_install(download_url: String) -> Result<()> {
    install_core_from_url(&download_url)
        .await
        .map_err(TheseusSerializableError::Amberite)
}

#[tauri::command]
async fn core_start() -> Result<String> {
    start_managed_core()
        .await
        .map_err(TheseusSerializableError::Amberite)
}

#[tauri::command]
async fn core_stop() -> Result<()> {
    stop_managed_core()
        .await
        .map_err(TheseusSerializableError::Amberite)
}

/// Get the effective Core base HTTP URL (used by the frontend to build WebSocket URLs).
#[tauri::command]
async fn core_get_url() -> String {
    resolve_core_url().await
}
