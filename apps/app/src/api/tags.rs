use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::tags::{Category, DonationPlatform, GameVersion, Loader};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("tags")
        .invoke_handler(tauri::generate_handler![
            tags_get_categories,
            tags_get_report_types,
            tags_get_loaders,
            tags_get_game_versions,
            tags_get_donation_platforms,
        ])
        .build()
}

/// Gets cached category tags from the database
#[tauri::command]
pub async fn tags_get_categories(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<Category>> {
    let state = handle.get().await;
    Ok(engine::tags::get_category_tags(&state).await?)
}

/// Gets cached report type tags from the database
#[tauri::command]
pub async fn tags_get_report_types(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<String>> {
    let state = handle.get().await;
    Ok(engine::tags::get_report_type_tags(&state).await?)
}

/// Gets cached loader tags from the database
#[tauri::command]
pub async fn tags_get_loaders(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<Loader>> {
    let state = handle.get().await;
    Ok(engine::tags::get_loader_tags(&state).await?)
}

/// Gets cached game version tags from the database
#[tauri::command]
pub async fn tags_get_game_versions(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<GameVersion>> {
    let state = handle.get().await;
    Ok(engine::tags::get_game_version_tags(&state).await?)
}

/// Gets cached donation platform tags from the database
#[tauri::command]
pub async fn tags_get_donation_platforms(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<DonationPlatform>> {
    let state = handle.get().await;
    Ok(engine::tags::get_donation_platform_tags(&state).await?)
}
