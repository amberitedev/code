use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::prelude::*;
use uuid::Uuid;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("process")
        .invoke_handler(tauri::generate_handler![
            process_get_all,
            process_get_by_instance_id,
            process_kill,
            process_wait_for,
        ])
        .build()
}

#[tauri::command]
pub async fn process_get_all(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<ProcessMetadata>> {
    let state = handle.get().await;
    Ok(process::get_all(&state).await?)
}

#[tauri::command]
pub async fn process_get_by_instance_id(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<Vec<ProcessMetadata>> {
    let state = handle.get().await;
    Ok(process::get_by_instance_id(&state, instance_id).await?)
}

#[tauri::command]
pub async fn process_kill(
    handle: tauri::State<'_, EngineHandle>,
    uuid: Uuid,
) -> Result<()> {
    let state = handle.get().await;
    Ok(process::kill(&state, uuid).await?)
}

#[tauri::command]
pub async fn process_wait_for(
    handle: tauri::State<'_, EngineHandle>,
    uuid: Uuid,
) -> Result<()> {
    let state = handle.get().await;
    Ok(process::wait_for(&state, uuid).await?)
}
