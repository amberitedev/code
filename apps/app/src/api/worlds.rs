use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::instance::{self, QuickPlayType, get_full_path};
use engine::prelude::ProcessMetadata;
use engine::server_address::ServerAddress;
use engine::worlds;
use engine::worlds::{
    DisplayStatus, ProtocolVersion, ServerPackStatus, ServerStatus, World,
    WorldType, WorldWithInstance,
};
use enumset::EnumSet;
use tauri::Runtime;

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("worlds")
        .invoke_handler(tauri::generate_handler![
            get_recent_worlds,
            get_instance_worlds,
            get_singleplayer_world,
            set_world_display_status,
            rename_world,
            reset_world_icon,
            backup_world,
            delete_world,
            add_server_to_instance,
            ensure_managed_server_in_instance,
            edit_server_in_instance,
            remove_server_from_instance,
            desync_server,
            get_instance_protocol_version,
            get_server_status,
            start_join_singleplayer_world,
            start_join_server,
        ])
        .build()
}

#[tauri::command]
pub async fn get_recent_worlds(
    handle: tauri::State<'_, EngineHandle>,
    limit: usize,
    display_statuses: Option<EnumSet<DisplayStatus>>,
) -> Result<Vec<WorldWithInstance>> {
    let state = handle.get().await;
    Ok(worlds::get_recent_worlds(
        &state,
        limit,
        display_statuses.unwrap_or(EnumSet::all()),
    )
    .await?)
}

#[tauri::command]
pub async fn get_instance_worlds(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<Vec<World>> {
    let state = handle.get().await;
    Ok(worlds::get_instance_worlds(&state, instance_id).await?)
}

#[tauri::command]
pub async fn get_singleplayer_world(
    handle: tauri::State<'_, EngineHandle>,
    instance: &str,
    world: &str,
) -> Result<World> {
    let state = handle.get().await;
    Ok(worlds::get_singleplayer_world(&state, instance, world).await?)
}

#[tauri::command]
pub async fn set_world_display_status(
    handle: tauri::State<'_, EngineHandle>,
    instance: &str,
    world_type: WorldType,
    world_id: &str,
    display_status: DisplayStatus,
) -> Result<()> {
    let state = handle.get().await;
    Ok(worlds::set_world_display_status(
        &state,
        instance,
        world_type,
        world_id,
        display_status,
    )
    .await?)
}

#[tauri::command]
pub async fn rename_world(
    handle: tauri::State<'_, EngineHandle>,
    instance: &str,
    world: &str,
    new_name: &str,
) -> Result<()> {
    let state = handle.get().await;
    let instance = get_full_path(&state, instance).await?;
    worlds::rename_world(&instance, world, new_name).await?;
    Ok(())
}

#[tauri::command]
pub async fn reset_world_icon(
    handle: tauri::State<'_, EngineHandle>,
    instance: &str,
    world: &str,
) -> Result<()> {
    let state = handle.get().await;
    let instance = get_full_path(&state, instance).await?;
    worlds::reset_world_icon(&instance, world).await?;
    Ok(())
}

#[tauri::command]
pub async fn backup_world(
    handle: tauri::State<'_, EngineHandle>,
    instance: &str,
    world: &str,
) -> Result<u64> {
    let state = handle.get().await;
    let instance = get_full_path(&state, instance).await?;
    Ok(worlds::backup_world(&instance, world).await?)
}

#[tauri::command]
pub async fn delete_world(
    handle: tauri::State<'_, EngineHandle>,
    instance: &str,
    world: &str,
) -> Result<()> {
    let state = handle.get().await;
    let instance = get_full_path(&state, instance).await?;
    worlds::delete_world(&instance, world).await?;
    Ok(())
}

#[tauri::command]
pub async fn add_server_to_instance(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    name: String,
    address: String,
    pack_status: ServerPackStatus,
    project_id: Option<String>,
    content_kind: Option<String>,
) -> Result<usize> {
    let state = handle.get().await;
    Ok(worlds::add_server_to_instance(
        &state,
        instance_id,
        name,
        address,
        pack_status,
        project_id,
        content_kind,
    )
    .await?)
}

#[tauri::command]
pub async fn ensure_managed_server_in_instance(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    name: String,
    address: String,
) -> Result<()> {
    let state = handle.get().await;
    Ok(worlds::ensure_managed_server_in_instance(
        &state,
        instance_id,
        name,
        address,
    )
    .await?)
}

#[tauri::command]
pub async fn edit_server_in_instance(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    index: usize,
    name: String,
    address: String,
    pack_status: ServerPackStatus,
) -> Result<()> {
    let state = handle.get().await;
    worlds::edit_server_in_instance(
        &state,
        instance_id,
        index,
        name,
        address,
        pack_status,
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn remove_server_from_instance(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    index: usize,
) -> Result<()> {
    let state = handle.get().await;
    worlds::remove_server_from_instance(&state, instance_id, index).await?;
    Ok(())
}

#[tauri::command]
pub async fn desync_server(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    server_id: &str,
    mode: engine::instance::DesyncServerMode,
) -> Result<()> {
    let state = handle.get().await;
    Ok(
        engine::instance::desync_server(&state, instance_id, server_id, mode)
            .await?,
    )
}

#[tauri::command]
pub async fn get_instance_protocol_version(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<Option<ProtocolVersion>> {
    let state = handle.get().await;
    Ok(worlds::get_instance_protocol_version(&state, instance_id).await?)
}

#[tauri::command]
pub async fn get_server_status(
    address: &str,
    protocol_version: Option<ProtocolVersion>,
) -> Result<ServerStatus> {
    Ok(worlds::get_server_status(address, protocol_version).await?)
}

#[tauri::command]
pub async fn start_join_singleplayer_world(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    world: String,
) -> Result<ProcessMetadata> {
    let state = handle.get().await;
    let process =
        instance::run(&state, instance_id, QuickPlayType::Singleplayer(world))
            .await?;

    Ok(process)
}

#[tauri::command]
pub async fn start_join_server(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    address: &str,
) -> Result<ProcessMetadata> {
    let state = handle.get().await;
    let process = instance::run(
        &state,
        instance_id,
        QuickPlayType::Server(ServerAddress::Unresolved(address.to_owned())),
    )
    .await?;

    Ok(process)
}
