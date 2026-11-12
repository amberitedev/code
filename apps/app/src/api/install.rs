use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::data::ModLoader;
use engine::endpoints::instances::InstanceLink;
use engine::install::{
    InstallJobSnapshot, InstallModpackPreview, InstallPostInstallEdit,
};
use engine::instance::{
    SharedHistorySettings, SharedInstanceHistoryEntry,
    SharedInstanceInstallPreview, SharedInstanceInviteInstallPreview,
    SharedInstanceUpdatePreview,
};
use engine::pack::import::ImportLauncherType;
use engine::pack::install_from::CreatePackLocation;
use serde::Deserialize;
use std::path::PathBuf;
use uuid::Uuid;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("install")
        .invoke_handler(tauri::generate_handler![
            install_get_modpack_preview,
            install_create_instance,
            install_create_modpack_instance,
            install_get_shared_instance_preview,
            install_accept_shared_instance_invite,
            install_get_shared_instance_invite_preview,
            install_accept_pending_shared_instance_invite,
            install_get_shared_instance_update_preview,
            install_shared_instance,
            install_update_shared_instance,
            install_get_shared_instance_history,
            install_restore_shared_instance_version,
            install_delete_shared_instance_version,
            install_get_shared_history_settings,
            install_set_shared_history_kept,
            install_bulk_update_content,
            install_import_instance,
            install_duplicate_instance,
            install_existing_instance,
            install_pack_to_existing_instance,
            install_job_list,
            install_job_get,
            install_job_retry,
            install_job_pause,
            install_job_resume,
            install_job_cancel,
            install_job_dismiss,
            install_job_support_details,
        ])
        .build()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallCreateInstanceRequest {
    pub name: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub icon_path: Option<String>,
    pub icon_config: Option<engine::data::IconConfig>,
    pub link: Option<InstanceLink>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPostInstallEditRequest {
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub icon_path: Option<Option<String>>,
    pub link: Option<InstanceLink>,
}

impl InstallPostInstallEditRequest {
    fn into_core(self) -> Result<InstallPostInstallEdit> {
        Ok(InstallPostInstallEdit {
            name: self.name,
            icon_path: self.icon_path,
            link: self.link.map(|link| link.into_core()).transpose()?,
        })
    }
}

#[tauri::command]
pub async fn install_get_modpack_preview(
    handle: tauri::State<'_, EngineHandle>,
    location: CreatePackLocation,
) -> Result<InstallModpackPreview> {
    let state = handle.get().await;
    Ok(
        engine::pack::install_from::get_instance_from_pack(&state, location)
            .await?,
    )
}

#[tauri::command]
pub async fn install_create_instance(
    handle: tauri::State<'_, EngineHandle>,
    request: InstallCreateInstanceRequest,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::create_instance(
        request.name.trim().to_string(),
        request.game_version,
        request.loader,
        request.loader_version,
        request.icon_path,
        request.icon_config,
        match request.link {
            Some(link) => link.into_core()?,
            None => engine::data::InstanceLink::Unmanaged,
        },
        &state,
    )
    .await?)
}

#[tauri::command]
pub async fn install_create_modpack_instance(
    handle: tauri::State<'_, EngineHandle>,
    location: CreatePackLocation,
    post_install_edit: Option<InstallPostInstallEditRequest>,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::create_modpack_instance(
        location,
        post_install_edit.map(|edit| edit.into_core()).transpose()?,
        &state,
    )
    .await?)
}

#[tauri::command]
pub async fn install_get_shared_instance_preview(
    handle: tauri::State<'_, EngineHandle>,
    shared_instance_id: String,
    name: String,
) -> Result<SharedInstanceInstallPreview> {
    let state = handle.get().await;
    Ok(engine::instance::get_shared_instance_install_preview(
        &state,
        &shared_instance_id,
        name,
    )
    .await?)
}

#[tauri::command]
pub async fn install_get_shared_instance_invite_preview(
    handle: tauri::State<'_, EngineHandle>,
    invite_id: String,
) -> Result<SharedInstanceInviteInstallPreview> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_shared_instance_invite_install_preview(
            &state, &invite_id,
        )
        .await?,
    )
}

#[tauri::command]
pub async fn install_accept_shared_instance_invite(
    handle: tauri::State<'_, EngineHandle>,
    shared_instance_id: String,
    invite_id: String,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::accept_shared_instance_invite_for_install(
        &state,
        &shared_instance_id,
        &invite_id,
    )
    .await?)
}

#[tauri::command]
pub async fn install_accept_pending_shared_instance_invite(
    handle: tauri::State<'_, EngineHandle>,
    shared_instance_id: String,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::accept_pending_shared_instance_invite(
        &state,
        &shared_instance_id,
    )
    .await?)
}

#[tauri::command]
pub async fn install_get_shared_instance_update_preview(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: String,
) -> Result<Option<SharedInstanceUpdatePreview>> {
    let state = handle.get().await;
    Ok(engine::instance::get_shared_instance_update_preview(
        &state,
        &instance_id,
    )
    .await?)
}

#[tauri::command]
pub async fn install_shared_instance(
    handle: tauri::State<'_, EngineHandle>,
    shared_instance_id: String,
    name: String,
    manager_id: Option<String>,
    server_manager_name: Option<String>,
    server_manager_icon_url: Option<String>,
    instance_icon_url: Option<String>,
    instance_icon_config: Option<engine::data::IconConfig>,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::instance::install_shared_instance(
        &state,
        &shared_instance_id,
        name,
        manager_id,
        server_manager_name,
        server_manager_icon_url,
        instance_icon_url,
        instance_icon_config,
    )
    .await?)
}

#[tauri::command]
pub async fn install_update_shared_instance(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: String,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::instance::update_shared_instance(&state, &instance_id).await?)
}

#[tauri::command]
pub async fn install_get_shared_instance_history(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: String,
) -> Result<Vec<SharedInstanceHistoryEntry>> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_shared_instance_history(&state, &instance_id)
            .await?,
    )
}

#[tauri::command]
pub async fn install_restore_shared_instance_version(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: String,
    version: i32,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::instance::restore_shared_instance_version(
        &state,
        &instance_id,
        version,
    )
    .await?)
}

#[tauri::command]
pub async fn install_delete_shared_instance_version(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: String,
    version: i32,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::delete_shared_instance_version(
        &state,
        &instance_id,
        version,
    )
    .await?)
}

#[tauri::command]
pub async fn install_get_shared_history_settings(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<SharedHistorySettings> {
    let state = handle.get().await;
    Ok(engine::instance::get_shared_history_settings(&state).await?)
}

#[tauri::command]
pub async fn install_set_shared_history_kept(
    handle: tauri::State<'_, EngineHandle>,
    kept: Option<usize>,
) -> Result<SharedHistorySettings> {
    let state = handle.get().await;
    Ok(engine::instance::set_shared_history_kept(&state, kept).await?)
}

#[tauri::command]
pub async fn install_bulk_update_content(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: String,
    updates: Vec<engine::install::ContentUpdateSelection>,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(
        engine::install::bulk_update_content(instance_id, updates, &state)
            .await?,
    )
}

#[tauri::command]
pub async fn install_import_instance(
    handle: tauri::State<'_, EngineHandle>,
    launcher_type: ImportLauncherType,
    base_path: PathBuf,
    instance_folder: String,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::import_instance(
        launcher_type,
        base_path,
        instance_folder,
        &state,
    )
    .await?)
}

#[tauri::command]
pub async fn install_duplicate_instance(
    handle: tauri::State<'_, EngineHandle>,
    source_instance_id: String,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::duplicate_instance(source_instance_id, &state).await?)
}

#[tauri::command]
pub async fn install_existing_instance(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: String,
    force: bool,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(
        engine::install::install_existing_instance(instance_id, force, &state)
            .await?,
    )
}

#[tauri::command]
pub async fn install_pack_to_existing_instance(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: String,
    location: CreatePackLocation,
    post_install_edit: Option<InstallPostInstallEditRequest>,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::install_pack_to_existing_instance(
        instance_id,
        location,
        post_install_edit.map(|edit| edit.into_core()).transpose()?,
        &state,
    )
    .await?)
}

#[tauri::command]
pub async fn install_job_list(
    handle: tauri::State<'_, EngineHandle>,
    include_finished: bool,
) -> Result<Vec<InstallJobSnapshot>> {
    let state = handle.get().await;
    Ok(engine::install::list_jobs(include_finished, &state).await?)
}

#[tauri::command]
pub async fn install_job_get(
    handle: tauri::State<'_, EngineHandle>,
    job_id: Uuid,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::get_job(job_id, &state).await?)
}

#[tauri::command]
pub async fn install_job_retry(
    handle: tauri::State<'_, EngineHandle>,
    job_id: Uuid,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::retry_job(job_id, &state).await?)
}

#[tauri::command]
pub async fn install_job_pause(
    handle: tauri::State<'_, EngineHandle>,
    job_id: Uuid,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::pause_job(job_id, &state).await?)
}

#[tauri::command]
pub async fn install_job_resume(
    handle: tauri::State<'_, EngineHandle>,
    job_id: Uuid,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::resume_job(job_id, &state).await?)
}

#[tauri::command]
pub async fn install_job_cancel(
    handle: tauri::State<'_, EngineHandle>,
    job_id: Uuid,
) -> Result<InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::install::cancel_job(job_id, &state).await?)
}

#[tauri::command]
pub async fn install_job_dismiss(
    handle: tauri::State<'_, EngineHandle>,
    job_id: Uuid,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::install::dismiss_job(job_id, &state).await?)
}

#[tauri::command]
pub async fn install_job_support_details(
    handle: tauri::State<'_, EngineHandle>,
    job_id: Uuid,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::install::job_support_details(job_id, &state).await?)
}
