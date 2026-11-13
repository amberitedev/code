use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::prelude::*;
use tauri::Runtime;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("settings")
        .invoke_handler(tauri::generate_handler![
            settings_get,
            settings_set,
            store_usage,
            store_cleanup,
            store_set_cache_limit,
            cancel_directory_change
        ])
        .build()
}

#[tauri::command]
pub async fn store_usage(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<settings::StoreUsage> {
    let state = handle.get().await;
    Ok(settings::store_usage(&state).await?)
}

#[tauri::command]
pub async fn store_cleanup(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<u64> {
    let state = handle.get().await;
    Ok(settings::store_cleanup(&state).await?)
}

#[tauri::command]
pub async fn store_set_cache_limit(
    handle: tauri::State<'_, EngineHandle>,
    bytes: u64,
) -> Result<()> {
    let state = handle.get().await;
    Ok(settings::store_set_cache_limit(&state, bytes).await?)
}

// Get full settings
// invoke('plugin:settings|settings_get')
#[tauri::command]
pub async fn settings_get(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Settings> {
    let state = handle.get().await;
    let res = settings::get(&state).await?;
    Ok(res)
}

// Set full settings
// invoke('plugin:settings|settings_set', settings)
#[tauri::command]
pub async fn settings_set(
    handle: tauri::State<'_, EngineHandle>,
    settings: Settings,
) -> Result<()> {
    let state = handle.get().await;
    settings::set(&state, settings).await?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_directory_change<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<()> {
    let identifier = &app.config().identifier;
    crate::data_dir::cancel_directory_change(identifier).await?;
    Ok(())
}
