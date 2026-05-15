use crate::api::Result;
use crate::api::TheseusSerializableError;
use amberite_lib::error::AmberiteError;
use theseus::prelude::*;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("profile-create")
        .invoke_handler(tauri::generate_handler![
            profile_create,
            profile_duplicate
        ])
        .build()
}

/// Creates a profile. Core instance provisioning is owned by the frontend Core API client.
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
    core_instance_id: Option<String>,
) -> Result<String> {
    let _ = port;

    let profile_kind = match kind.as_deref() {
        Some("server") => Some(ProfileKind::Server),
        Some("synced") => Some(ProfileKind::Synced),
        _ => Some(ProfileKind::Client),
    };

    let requires_core_id = matches!(
        profile_kind,
        Some(ProfileKind::Server) | Some(ProfileKind::Synced)
    );
    if requires_core_id && core_instance_id.is_none() {
        return Err(TheseusSerializableError::Amberite(AmberiteError::Other(
            "Server profiles require a Core instance ID".into(),
        )));
    }

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
