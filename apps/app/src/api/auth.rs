use crate::api::Result;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::env;
use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime, UserAttentionType};
use theseus::prelude::*;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("auth")
        .invoke_handler(tauri::generate_handler![
            check_reachable,
            login,
            remove_user,
            get_default_user,
            set_default_user,
            get_users,
            amberite_login,
            get_amberite_session_jwt,
            set_amberite_session_jwt,
            get_amberite_session_refresh_token,
            set_amberite_session_refresh_token,
            get_amberite_local_setup_secret,
        ])
        .build()
}

const AMBERITE_KEYRING_SERVICE: &str = "dev.amberite.app";
const AMBERITE_SESSION_JWT_ACCOUNT: &str = "amberite-session-jwt";
const AMBERITE_SESSION_REFRESH_TOKEN_ACCOUNT: &str =
    "amberite-session-refresh-token";
const AMBERITE_LOCAL_CORE_DATA_DIR_ENV: &str = "AMBERITE_LOCAL_CORE_DATA_DIR";

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftCredential {
    access_token: String,
    username: String,
    uuid: String,
}

fn other_error(
    error: impl std::fmt::Display,
) -> crate::api::TheseusSerializableError {
    theseus::ErrorKind::OtherError(error.to_string())
        .as_error()
        .into()
}

fn amberite_session_jwt_account() -> Result<String> {
    Ok(match crate::dev::config() {
        Some(config) => {
            format!("{AMBERITE_SESSION_JWT_ACCOUNT}:dev:{}", config.app_id)
        }
        None => AMBERITE_SESSION_JWT_ACCOUNT.to_string(),
    })
}

fn amberite_session_refresh_token_account() -> Result<String> {
    Ok(match crate::dev::config() {
        Some(config) => format!(
            "{AMBERITE_SESSION_REFRESH_TOKEN_ACCOUNT}:dev:{}",
            config.app_id
        ),
        None => AMBERITE_SESSION_REFRESH_TOKEN_ACCOUNT.to_string(),
    })
}

/// Returns the active Minecraft credentials so the frontend can trade them for an
/// Amberite session with Convex Auth. The credentials are refreshed if expired.
#[tauri::command]
pub async fn amberite_login() -> Result<MinecraftCredential> {
    let credentials = minecraft_auth::default_credential()
        .await
        .map_err(other_error)?
        .ok_or_else(|| other_error("No Minecraft account is signed in"))?;

    Ok(MinecraftCredential {
        access_token: credentials.access_token,
        username: credentials.offline_profile.name,
        uuid: credentials.offline_profile.id.to_string(),
    })
}

#[tauri::command]
pub async fn get_amberite_session_jwt() -> Result<Option<String>> {
    let account = amberite_session_jwt_account()?;
    let entry = keyring::Entry::new(AMBERITE_KEYRING_SERVICE, &account)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[tauri::command]
pub async fn set_amberite_session_jwt(jwt: Option<String>) -> Result<()> {
    let account = amberite_session_jwt_account()?;
    let entry = keyring::Entry::new(AMBERITE_KEYRING_SERVICE, &account)?;
    match jwt {
        Some(jwt) if !jwt.trim().is_empty() => entry.set_password(&jwt)?,
        _ => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error.into()),
        },
    }
    Ok(())
}

#[tauri::command]
pub async fn get_amberite_session_refresh_token() -> Result<Option<String>> {
    let account = amberite_session_refresh_token_account()?;
    let entry = keyring::Entry::new(AMBERITE_KEYRING_SERVICE, &account)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[tauri::command]
pub async fn set_amberite_session_refresh_token(
    refresh_token: Option<String>,
) -> Result<()> {
    let account = amberite_session_refresh_token_account()?;
    let entry = keyring::Entry::new(AMBERITE_KEYRING_SERVICE, &account)?;
    match refresh_token {
        Some(refresh_token) if !refresh_token.trim().is_empty() => {
            entry.set_password(&refresh_token)?
        }
        _ => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error.into()),
        },
    }
    Ok(())
}

#[tauri::command]
pub async fn get_amberite_local_setup_secret() -> Result<Option<String>> {
    for data_dir in local_core_data_dir_candidates() {
        let path = data_dir.join(".setup_secret");
        let Ok(value) = tokio::fs::read_to_string(&path).await else {
            continue;
        };
        let secret = value.trim().to_string();
        if !secret.is_empty() {
            return Ok(Some(secret));
        }
    }
    Ok(None)
}

fn local_core_data_dir_candidates() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(value) = env::var(AMBERITE_LOCAL_CORE_DATA_DIR_ENV) {
        let value = value.trim();
        if !value.is_empty() {
            candidates.push(std::path::PathBuf::from(value));
        }
    }
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join(".copal"));
        candidates.push(cwd.join("apps").join("core").join(".copal"));
        candidates.push(cwd.join("..").join("core").join(".copal"));
    }
    candidates
}

/// Checks if the authentication servers are reachable.
#[tauri::command]
pub async fn check_reachable() -> Result<()> {
    minecraft_auth::check_reachable().await?;
    Ok(())
}

/// Authenticate a user with Hydra - part 1
/// This begins the authentication flow quasi-synchronously, returning a URL to visit (that the user will sign in at)
#[tauri::command]
pub async fn login<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<Credentials>> {
    let flow = minecraft_auth::begin_login().await?;

    let start = Utc::now();

    if let Some(window) = app.get_webview_window("signin") {
        window.close()?;
    }

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "signin",
        tauri::WebviewUrl::External(flow.auth_request_uri.parse().map_err(
            |_| {
                theseus::ErrorKind::OtherError(
                    "Error parsing auth redirect URL".to_string(),
                )
                .as_error()
            },
        )?),
    )
    .title("Sign into Modrinth")
    .always_on_top(true)
    .center()
    .build()?;

    window.request_user_attention(Some(UserAttentionType::Critical))?;

    while (Utc::now() - start) < Duration::minutes(10) {
        if window.title().is_err() {
            // user closed window, cancelling flow
            return Ok(None);
        }

        if window
            .url()?
            .as_str()
            .starts_with("https://login.live.com/oauth20_desktop.srf")
            && let Some((_, code)) =
                window.url()?.query_pairs().find(|x| x.0 == "code")
        {
            window.close()?;
            let val = minecraft_auth::finish_login(&code.clone(), flow).await?;

            return Ok(Some(val));
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    window.close()?;
    Ok(None)
}

#[tauri::command]
pub async fn remove_user(user: uuid::Uuid) -> Result<()> {
    Ok(minecraft_auth::remove_user(user).await?)
}

#[tauri::command]
pub async fn get_default_user() -> Result<Option<uuid::Uuid>> {
    Ok(minecraft_auth::get_default_user().await?)
}

#[tauri::command]
pub async fn set_default_user(user: uuid::Uuid) -> Result<()> {
    Ok(minecraft_auth::set_default_user(user).await?)
}

/// Get a copy of the list of all user credentials
#[tauri::command]
pub async fn get_users() -> Result<Vec<Credentials>> {
    Ok(minecraft_auth::users().await?)
}
