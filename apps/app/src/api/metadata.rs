use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::metadata::{Manifest, VersionManifest};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("metadata")
        .invoke_handler(tauri::generate_handler![
            metadata_get_game_versions,
            metadata_get_loader_versions,
        ])
        .build()
}

/// Gets the game versions from launcher metadata
#[tauri::command]
pub async fn metadata_get_game_versions(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<VersionManifest> {
    let state = handle.get().await;
    Ok(engine::metadata::get_minecraft_versions(&state).await?)
}

/// Gets the fabric versions from launcher metadata
#[tauri::command]
pub async fn metadata_get_loader_versions(
    handle: tauri::State<'_, EngineHandle>,
    loader: &str,
) -> Result<Manifest> {
    let state = handle.get().await;
    Ok(engine::metadata::get_loader_versions(&state, loader).await?)
}
