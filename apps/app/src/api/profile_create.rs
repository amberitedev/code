use crate::api::Result;
use amberite_lib::core_instances::{CreateInstanceRequest, create_instance};
use amberite_lib::settings::AppSettings;
use reqwest::Client;
use theseus::prelude::*;
use crate::api::TheseusSerializableError;

const DEV_CORE_URL: &str = "http://localhost:16662";

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("profile-create")
        .invoke_handler(tauri::generate_handler![
            profile_create,
            profile_duplicate
        ])
        .build()
}

/// Creates a profile and, for server/synced kinds, provisions a Core instance.
// invoke('plugin:profile-create|profile_create', profile)
#[tauri::command]
pub async fn profile_create(
    name: String,
    game_version: String,
    modloader: ModLoader,
    loader_version: Option<String>,
    icon: Option<String>,
    skip_install: Option<bool>,
    linked_data: Option<LinkedData>,
    // "client" | "server" | "synced" — defaults to "client"
    kind: Option<String>,
    port: Option<u16>,
) -> Result<String> {
    let profile_kind = match kind.as_deref() {
        Some("server") => Some(ProfileKind::Server),
        Some("synced") => Some(ProfileKind::Synced),
        _ => Some(ProfileKind::Client),
    };

    // For server/synced: provision a Core instance first to get its ID
    let core_instance_id = if matches!(profile_kind, Some(ProfileKind::Server) | Some(ProfileKind::Synced)) {
        let settings = AppSettings::load().await.ok();
        let core_url = settings
            .and_then(|s| s.core_url)
            .unwrap_or_else(|| DEV_CORE_URL.to_string());
        let core_url_opt = Some(core_url);

        let client = Client::new();
        let req = CreateInstanceRequest {
            name: name.clone(),
            game_version: game_version.clone(),
            loader: modloader.as_str().to_string(),
            loader_version: loader_version.clone(),
            port: port.unwrap_or(25565),
            memory: None,
        };
        let detail = create_instance(&client, &core_url_opt, req).await
            .map_err(TheseusSerializableError::Amberite)?;
        Some(detail.id)
    } else {
        None
    };

    let res = profile::create::profile_create(
        name,
        game_version,
        modloader,
        loader_version,
        icon,
        linked_data,
        skip_install,
        profile_kind,
        core_instance_id,
    )
    .await?;
    Ok(res)
}

// Creates a profile from a duplicate
// invoke('plugin:profile-create|profile_duplicate', profile)
#[tauri::command]
pub async fn profile_duplicate(path: &str) -> Result<String> {
    let res = profile::create::profile_create_from_duplicate(path).await?;
    Ok(res)
}
