use crate::api::Result;
use crate::engine_handle::EngineHandle;
use anyhow::Context;
use chrono::{Duration, Utc};
use engine::prelude::*;
use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime, UserAttentionType};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("auth")
        .invoke_handler(tauri::generate_handler![
            check_reachable,
            login,
            remove_user,
            get_default_user,
            set_default_user,
            get_users,
        ])
        .build()
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
    handle: tauri::State<'_, EngineHandle>,
    app: tauri::AppHandle<R>,
) -> Result<Option<Credentials>> {
    let state = handle.get().await;
    let flow = minecraft_auth::begin_login(&state).await?;

    let start = Utc::now();

    if let Some(window) = app.get_webview_window("signin") {
        window.close()?;
    }

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "signin",
        tauri::WebviewUrl::External(
            flow.auth_request_uri
                .parse()
                .context("error parsing auth redirect URL")?,
        ),
    )
    .title("Sign into Modrinth")
    .always_on_top(true)
    .min_inner_size(500.0, 500.0)
    .inner_size(1000.0, 700.0)
    .focused(true)
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
            let val = minecraft_auth::finish_login(&state, &code.clone(), flow)
                .await?;

            return Ok(Some(val));
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    window.close()?;
    Ok(None)
}

#[tauri::command]
pub async fn remove_user(
    handle: tauri::State<'_, EngineHandle>,
    user: uuid::Uuid,
) -> Result<()> {
    let state = handle.get().await;
    Ok(minecraft_auth::remove_user(&state, user).await?)
}

#[tauri::command]
pub async fn get_default_user(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Option<uuid::Uuid>> {
    let state = handle.get().await;
    Ok(minecraft_auth::get_default_user(&state).await?)
}

#[tauri::command]
pub async fn set_default_user(
    handle: tauri::State<'_, EngineHandle>,
    user: uuid::Uuid,
) -> Result<()> {
    let state = handle.get().await;
    Ok(minecraft_auth::set_default_user(&state, user).await?)
}

/// Get the account summaries displayed by the launcher.
#[tauri::command]
pub async fn get_users(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<minecraft_auth::MinecraftAccountSummary>> {
    let state = handle.get().await;
    Ok(minecraft_auth::users(&state).await?)
}
