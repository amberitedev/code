use crate::api::Result;
use crate::engine_handle::EngineHandle;
use dashmap::DashMap;
use engine::DownloadReason;
use engine::data::{
    AppliedContentSetPatch, ContentItem, Dependency,
    EditInstance as CoreEditInstance, InstanceInstallCandidate,
    InstanceInstallTarget, InstanceLaunchOverridesPatch,
    InstanceLink as CoreInstanceLink, InstanceTabVisibility, LinkedModpackInfo,
};
use engine::endpoints::instances::{
    Instance, InstanceLink, SharedInstanceAttachment,
};
use engine::instance::InstallProjectWithDependenciesRequest;
use engine::instance::QuickPlayType;
use engine::prelude::*;
use engine::server_address::ServerAddress;
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("instance")
        .invoke_handler(tauri::generate_handler![
            instance_remove,
            instance_get_projects,
            instance_get_installed_project_ids,
            instance_get_install_candidates,
            instance_content,
            instance_get_content_items,
            instance_sync_content_files,
            instance_refresh_content_updates,
            instance_get_dependencies_as_content_items,
            instance_get_linked_modpack_info,
            instance_get_linked_modpack_content,
            instance_get_optimal_jre_key,
            instance_get_full_path,
            instance_get_mod_full_path,
            instance_list_screenshots,
            instance_list_all_screenshots,
            instance_list_synced_screenshots,
            instance_save_edited_screenshot,
            instance_list_screenshot_groups,
            instance_create_screenshot_group,
            instance_rename_screenshot_group,
            instance_delete_screenshot_group,
            instance_set_screenshot_group_memberships,
            instance_import_screenshot_groups,
            instance_delete_screenshots,
            instance_export_screenshots,
            instance_move_screenshots,
            instance_set_synced_option,
            instance_get_synced_option_join_preview,
            instance_get_synced_options_overview,
            instance_get_global_synced_options,
            instance_get_initialized_synced_options,
            instance_set_global_synced_option,
            instance_list_game_options_sync_sources,
            instance_get_synced_game_options_config,
            instance_get_game_setting_locale_labels,
            instance_preview_synced_game_option_changes,
            instance_save_synced_game_option_changes,
            instance_get_local_game_options_config,
            instance_preview_local_game_option_changes,
            instance_save_local_game_option_changes,
            instance_get_command_history,
            instance_set_command_history,
            instance_list_synced_servers,
            instance_update_synced_server,
            instance_remove_synced_server,
            instance_get_pack_sync_preview,
            instance_sync_pack,
            instance_desync_pack,
            instance_list_synced_packs,
            instance_upload_synced_pack,
            instance_set_synced_pack_enabled,
            instance_remove_synced_pack,
            instance_rebuild_synced_options,
            instance_check_installed,
            instance_update_project,
            instance_add_project_from_version,
            instance_install_project_with_dependencies,
            instance_switch_project_version_with_dependencies,
            instance_add_project_from_path,
            instance_is_file_on_modrinth,
            instance_toggle_disable_project,
            instance_set_project_locked,
            instance_remove_project,
            instance_update_managed_modrinth_version,
            instance_repair_managed_modrinth,
            instance_run,
            instance_kill,
            instance_edit,
            instance_edit_icon,
            instance_cache_icon,
            instance_edit_generated_icon,
            instance_cache_generated_icon,
            instance_share_can_current_user_use,
            instance_share_get_users,
            instance_share_invite_users,
            instance_share_create_invite_link,
            instance_share_get_invites,
            instance_share_revoke_invite,
            instance_share_remove_users,
            instance_share_get_publish_preview,
            instance_share_publish,
            instance_share_publish_support_details,
            instance_share_unlink,
            instance_share_unpublish,
            instance_share_ensure,
            instance_share_unpublish_remote,
            instance_share_leave_remote,
            instance_share_get_cached_library,
            instance_share_cache_library,
            instance_get_hidden_ids,
            instance_set_hidden,
            instance_share_dismiss_restored_notice,
            instance_export_mrpack,
            instance_get_pack_export_candidates,
        ])
        .build()
}

#[derive(Serialize, Debug, Clone)]
pub struct InstanceScreenshot {
    pub id: String,
    pub instance_id: String,
    pub instance_name: String,
    pub file_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: i64,
    pub group_id: Option<String>,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EditInstance {
    pub name: Option<String>,

    pub game_version: Option<String>,
    pub loader: Option<ModLoader>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub loader_version: Option<Option<String>>,

    pub group_ids: Option<Vec<String>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub link: Option<Option<InstanceLink>>,
    pub update_channel: Option<ReleaseChannel>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub java_path: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub extra_launch_args: Option<Option<Vec<String>>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub custom_env_vars: Option<Option<Vec<(String, String)>>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub memory: Option<Option<MemorySettings>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub force_fullscreen: Option<Option<bool>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub game_resolution: Option<Option<WindowSize>>,
    pub hooks: Option<Hooks>,
    pub visible_tabs: Option<InstanceTabVisibility>,
}

fn edit_to_core(edit_instance: EditInstance) -> Result<CoreEditInstance> {
    Ok(CoreEditInstance {
        install_stage: None,
        launcher_feature_version: None,
        name: edit_instance.name,
        icon_path: None,
        icon_config: None,
        update_channel: edit_instance.update_channel,
        group_ids: edit_instance.group_ids,
        link: edit_instance
            .link
            .map(|link| match link {
                Some(link) => link.into_core(),
                None => Ok(CoreInstanceLink::Unmanaged),
            })
            .transpose()?,
        launch_overrides: Some(InstanceLaunchOverridesPatch {
            java_path: edit_instance.java_path,
            extra_launch_args: edit_instance.extra_launch_args,
            custom_env_vars: edit_instance.custom_env_vars,
            memory: edit_instance.memory,
            force_fullscreen: edit_instance.force_fullscreen,
            game_resolution: edit_instance.game_resolution,
            hooks: edit_instance.hooks,
            visible_tabs: edit_instance.visible_tabs,
        }),
        content_set_patch: Some(AppliedContentSetPatch {
            source_kind: None,
            game_version: edit_instance.game_version,
            protocol_version: Some(None),
            loader: edit_instance.loader,
            loader_version: edit_instance.loader_version,
        }),
        last_played: None,
        submitted_time_played: None,
        recent_time_played: None,
    })
}

#[tauri::command]
pub async fn instance_remove(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::remove(&state, instance_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_get_projects(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<DashMap<String, ContentFile>> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_projects(&state, instance_id, cache_behaviour)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_get_installed_project_ids(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<Vec<String>> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_installed_project_ids(&state, instance_id)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_get_install_candidates(
    handle: tauri::State<'_, EngineHandle>,
    project_id: &str,
    project_type: ProjectType,
    targets: Vec<InstanceInstallTarget>,
) -> Result<Vec<InstanceInstallCandidate>> {
    let state = handle.get().await;
    Ok(engine::instance::get_install_candidates(
        &state,
        project_id,
        project_type,
        targets,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_content(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Vec<ContentItem>> {
    instance_get_content_items(handle, instance_id, cache_behaviour).await
}

#[tauri::command]
pub async fn instance_get_content_items(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Vec<ContentItem>> {
    let state = handle.get().await;
    Ok(engine::instance::get_content_items(
        &state,
        instance_id,
        cache_behaviour,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_sync_content_files(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::sync_content_files(&state, instance_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_refresh_content_updates(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::refresh_content_updates(&state, instance_id).await?)
}

#[tauri::command]
pub async fn instance_get_dependencies_as_content_items(
    handle: tauri::State<'_, EngineHandle>,
    dependencies: Vec<Dependency>,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Vec<ContentItem>> {
    let state = handle.get().await;
    Ok(engine::instance::get_dependencies_as_content_items(
        &state,
        dependencies,
        cache_behaviour,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_get_linked_modpack_info(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Option<LinkedModpackInfo>> {
    let state = handle.get().await;
    Ok(engine::instance::get_linked_modpack_info(
        &state,
        instance_id,
        cache_behaviour,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_get_linked_modpack_content(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Vec<ContentItem>> {
    let state = handle.get().await;
    Ok(engine::instance::get_linked_modpack_content(
        &state,
        instance_id,
        cache_behaviour,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_get_full_path(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<PathBuf> {
    let state = handle.get().await;
    Ok(engine::instance::get_full_path(&state, instance_id).await?)
}

#[tauri::command]
pub async fn instance_get_mod_full_path(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_path: &str,
) -> Result<PathBuf> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_mod_full_path(&state, instance_id, project_path)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_list_screenshots(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<Vec<InstanceScreenshot>> {
    let state = handle.get().await;
    let screenshots =
        engine::instance::list_screenshots(&state, instance_id).await?;
    Ok(serialize_screenshots(screenshots))
}

#[tauri::command]
pub async fn instance_list_all_screenshots(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<InstanceScreenshot>> {
    let state = handle.get().await;
    let screenshots = engine::instance::list_all_screenshots(&state).await?;
    Ok(serialize_screenshots(screenshots))
}

#[tauri::command]
pub async fn instance_list_synced_screenshots(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<InstanceScreenshot>> {
    let state = handle.get().await;
    let screenshots = engine::instance::list_synced_screenshots(&state).await?;
    Ok(serialize_screenshots(screenshots))
}

#[tauri::command]
pub async fn instance_save_edited_screenshot(
    handle: tauri::State<'_, EngineHandle>,
    key: engine::instance::ScreenshotKey,
    png_bytes: Vec<u8>,
    mode: engine::instance::ScreenshotEditSaveMode,
) -> Result<InstanceScreenshot> {
    let state = handle.get().await;
    let screenshot =
        engine::instance::save_edited_screenshot(&state, key, png_bytes, mode)
            .await?;
    Ok(serialize_screenshot_data(screenshot))
}

#[tauri::command]
pub async fn instance_list_screenshot_groups(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<engine::instance::ScreenshotGroup>> {
    let state = handle.get().await;
    Ok(engine::instance::list_screenshot_groups(&state).await?)
}

#[tauri::command]
pub async fn instance_create_screenshot_group(
    handle: tauri::State<'_, EngineHandle>,
    name: String,
    screenshot_ids: Vec<String>,
) -> Result<engine::instance::ScreenshotGroup> {
    let state = handle.get().await;
    Ok(
        engine::instance::create_screenshot_group(&state, name, screenshot_ids)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_rename_screenshot_group(
    handle: tauri::State<'_, EngineHandle>,
    id: String,
    new_name: String,
) -> Result<engine::instance::ScreenshotGroup> {
    let state = handle.get().await;
    Ok(engine::instance::rename_screenshot_group(&state, id, new_name).await?)
}

#[tauri::command]
pub async fn instance_delete_screenshot_group(
    handle: tauri::State<'_, EngineHandle>,
    id: String,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::delete_screenshot_group(&state, id).await?)
}

#[tauri::command]
pub async fn instance_set_screenshot_group_memberships(
    handle: tauri::State<'_, EngineHandle>,
    updates: Vec<engine::instance::ScreenshotGroupMembershipUpdate>,
) -> Result<()> {
    let state = handle.get().await;
    Ok(
        engine::instance::set_screenshot_group_memberships(&state, updates)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_import_screenshot_groups(
    handle: tauri::State<'_, EngineHandle>,
    groups: Vec<engine::instance::ScreenshotGroupImport>,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::import_screenshot_groups(&state, groups).await?)
}

#[tauri::command]
pub async fn instance_delete_screenshots(
    handle: tauri::State<'_, EngineHandle>,
    keys: Vec<engine::instance::ScreenshotKey>,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::delete_screenshots(&state, &keys).await?)
}

#[tauri::command]
pub async fn instance_export_screenshots(
    handle: tauri::State<'_, EngineHandle>,
    keys: Vec<engine::instance::ScreenshotKey>,
    export_path: PathBuf,
) -> Result<()> {
    let state = handle.get().await;
    Ok(
        engine::instance::export_screenshots(&state, &keys, export_path)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_move_screenshots(
    handle: tauri::State<'_, EngineHandle>,
    keys: Vec<engine::instance::ScreenshotKey>,
    target_instance_id: &str,
) -> Result<Vec<engine::instance::ScreenshotKey>> {
    let state = handle.get().await;
    Ok(
        engine::instance::move_screenshots(&state, &keys, target_instance_id)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_set_synced_option(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    option: InstanceSyncedOption,
    enabled: bool,
    resolution: Option<engine::instance::SyncedOptionJoinResolution>,
) -> Result<Instance> {
    let state = handle.get().await;
    Ok(Instance::from(
        engine::instance::set_synced_option(
            &state,
            instance_id,
            option,
            enabled,
            resolution,
        )
        .await?,
    ))
}

#[tauri::command]
pub async fn instance_get_synced_option_join_preview(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    option: InstanceSyncedOption,
) -> Result<engine::instance::SyncedOptionJoinPreview> {
    let state = handle.get().await;
    Ok(engine::instance::get_synced_option_join_preview(
        &state,
        instance_id,
        option,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_get_synced_options_overview(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<engine::instance::SyncedOptionsOverview> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_synced_options_overview(&state, instance_id)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_get_global_synced_options(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<engine::instance::GlobalSyncedOptions> {
    let state = handle.get().await;
    Ok(engine::instance::get_global_synced_options(&state).await?)
}

#[tauri::command]
pub async fn instance_get_initialized_synced_options(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<engine::instance::GlobalSyncedOptions> {
    let state = handle.get().await;
    Ok(engine::instance::get_initialized_synced_options(&state).await?)
}

#[tauri::command]
pub async fn instance_set_global_synced_option(
    handle: tauri::State<'_, EngineHandle>,
    option: InstanceSyncedOption,
    enabled: bool,
    base_instance_id: Option<String>,
) -> Result<engine::instance::GlobalSyncedOptions> {
    let state = handle.get().await;
    Ok(engine::instance::set_global_synced_option(
        &state,
        option,
        enabled,
        base_instance_id.as_deref(),
    )
    .await?)
}

#[tauri::command]
pub async fn instance_list_game_options_sync_sources(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<engine::instance::GameOptionsSourceCandidate>> {
    let state = handle.get().await;
    Ok(engine::instance::list_game_options_sync_sources(&state).await?)
}

#[tauri::command]
pub async fn instance_get_synced_game_options_config(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<engine::instance::GameSettingsEditorState> {
    let state = handle.get().await;
    Ok(engine::instance::get_synced_game_options_config(&state).await?)
}

#[tauri::command]
pub async fn instance_get_game_setting_locale_labels(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: Option<String>,
    locale: String,
    option_ids: Vec<String>,
    refresh_sources: bool,
) -> Result<engine::instance::GameSettingLocaleLabels> {
    let state = handle.get().await;
    Ok(engine::instance::get_game_setting_locale_labels(
        &state,
        instance_id.as_deref(),
        &locale,
        option_ids,
        refresh_sources,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_preview_synced_game_option_changes(
    handle: tauri::State<'_, EngineHandle>,
    request: engine::instance::UpdateGameSettingsRequest,
) -> Result<engine::instance::GameSettingsEditorState> {
    let state = handle.get().await;
    Ok(
        engine::instance::preview_synced_game_option_changes(&state, request)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_save_synced_game_option_changes(
    handle: tauri::State<'_, EngineHandle>,
    request: engine::instance::UpdateGameSettingsRequest,
) -> Result<engine::instance::SaveGameSettingsResult> {
    let state = handle.get().await;
    Ok(
        engine::instance::save_synced_game_option_changes(&state, request)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_get_local_game_options_config(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<engine::instance::GameSettingsEditorState> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_local_game_options_config(&state, instance_id)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_preview_local_game_option_changes(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    request: engine::instance::UpdateGameSettingsRequest,
) -> Result<engine::instance::GameSettingsEditorState> {
    let state = handle.get().await;
    Ok(engine::instance::preview_local_game_option_changes(
        &state,
        instance_id,
        request,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_save_local_game_option_changes(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    request: engine::instance::UpdateGameSettingsRequest,
) -> Result<engine::instance::SaveGameSettingsResult> {
    let state = handle.get().await;
    Ok(engine::instance::save_local_game_option_changes(
        &state,
        instance_id,
        request,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_get_command_history(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::instance::get_command_history(&state).await?)
}

#[tauri::command]
pub async fn instance_set_command_history(
    handle: tauri::State<'_, EngineHandle>,
    contents: &str,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::instance::set_command_history(&state, contents).await?)
}

#[tauri::command]
pub async fn instance_list_synced_servers(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<engine::instance::SyncedServer>> {
    let state = handle.get().await;
    Ok(engine::instance::list_synced_servers(&state).await?)
}

#[tauri::command]
pub async fn instance_update_synced_server(
    handle: tauri::State<'_, EngineHandle>,
    server: engine::instance::SyncedServer,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::update_synced_server(&state, server).await?)
}

#[tauri::command]
pub async fn instance_remove_synced_server(
    handle: tauri::State<'_, EngineHandle>,
    server_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::remove_synced_server(&state, server_id).await?)
}

#[tauri::command]
pub async fn instance_get_pack_sync_preview(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_path: &str,
) -> Result<engine::instance::PackSyncPreview> {
    let state = handle.get().await;
    Ok(engine::instance::get_pack_sync_preview(
        &state,
        instance_id,
        project_path,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_sync_pack(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_path: &str,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::sync_pack(&state, instance_id, project_path).await?)
}

#[tauri::command]
pub async fn instance_desync_pack(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    pack_id: &str,
    mode: engine::instance::DesyncServerMode,
) -> Result<()> {
    let state = handle.get().await;
    Ok(
        engine::instance::desync_pack(&state, instance_id, pack_id, mode)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_list_synced_packs(
    handle: tauri::State<'_, EngineHandle>,
    project_type: ProjectType,
) -> Result<Vec<ContentItem>> {
    let state = handle.get().await;
    Ok(engine::instance::list_synced_packs(&state, project_type).await?)
}

#[tauri::command]
pub async fn instance_upload_synced_pack(
    handle: tauri::State<'_, EngineHandle>,
    path: &Path,
    project_type: ProjectType,
    game_versions: Vec<String>,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::upload_synced_pack(
        &state,
        path,
        project_type,
        game_versions,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_set_synced_pack_enabled(
    handle: tauri::State<'_, EngineHandle>,
    pack_id: &str,
    enabled: bool,
) -> Result<()> {
    let state = handle.get().await;
    Ok(
        engine::instance::set_synced_pack_enabled(&state, pack_id, enabled)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_remove_synced_pack(
    handle: tauri::State<'_, EngineHandle>,
    pack_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::remove_synced_pack(&state, pack_id).await?)
}

#[tauri::command]
pub async fn instance_rebuild_synced_options(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: Option<&str>,
) -> Result<()> {
    let state = handle.get().await;
    if let Some(instance_id) = instance_id {
        engine::instance::reconcile_instance_synced_options(
            &state,
            instance_id,
        )
        .await?;
    } else {
        engine::instance::reconcile_all_synced_options(&state).await?;
    }
    Ok(())
}

fn serialize_screenshots(
    screenshots: Vec<engine::instance::InstanceScreenshot>,
) -> Vec<InstanceScreenshot> {
    screenshots
        .into_iter()
        .map(serialize_screenshot_data)
        .collect()
}

fn serialize_screenshot_data(
    screenshot: engine::instance::InstanceScreenshot,
) -> InstanceScreenshot {
    InstanceScreenshot {
        id: screenshot.id,
        instance_id: screenshot.instance_id,
        instance_name: screenshot.instance_name,
        file_name: screenshot.file_name,
        created_at: screenshot.created_at,
        modified_at: screenshot.modified_at,
        group_id: screenshot.group_id,
        path: screenshot.path,
    }
}

#[tauri::command]
pub async fn instance_get_optimal_jre_key(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<Option<JavaVersion>> {
    let state = handle.get().await;
    Ok(engine::instance::get_optimal_jre_key(&state, instance_id).await?)
}

#[tauri::command]
pub async fn instance_check_installed(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_id: &str,
) -> Result<bool> {
    let state = handle.get().await;
    let check_project_id = project_id;

    if let Ok(projects) =
        engine::instance::get_projects(&state, instance_id, None).await
    {
        Ok(projects.into_iter().any(|(_, project)| {
            project
                .metadata
                .as_ref()
                .is_some_and(|metadata| check_project_id == metadata.project_id)
        }))
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn instance_update_project(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_path: &str,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::instance::update_project(
        &state,
        instance_id,
        project_path,
        None,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_add_project_from_version(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::instance::add_project_from_version(
        &state,
        instance_id,
        version_id,
        reason,
        dependent_on_version_id,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_install_project_with_dependencies(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    request: InstallProjectWithDependenciesRequest,
) -> Result<ResolveContentPlan> {
    let state = handle.get().await;
    Ok(engine::instance::install_project_with_dependencies(
        &state,
        instance_id,
        request,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_switch_project_version_with_dependencies(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_path: &str,
    version_id: &str,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::instance::switch_project_version_with_dependencies(
        &state,
        instance_id,
        project_path,
        version_id,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_add_project_from_path(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_path: &Path,
    project_type: Option<ProjectType>,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::instance::add_project_from_path(
        &state,
        instance_id,
        project_path,
        project_type,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_is_file_on_modrinth(
    handle: tauri::State<'_, EngineHandle>,
    project_path: &Path,
) -> Result<bool> {
    let state = handle.get().await;
    Ok(engine::instance::is_file_on_modrinth(&state, project_path).await?)
}

#[tauri::command]
pub async fn instance_toggle_disable_project(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_path: &str,
    desired_enabled: Option<bool>,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::instance::toggle_disable_project(
        &state,
        instance_id,
        project_path,
        desired_enabled,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_set_project_locked(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_path: &str,
    locked: bool,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::set_project_locked(
        &state,
        instance_id,
        project_path,
        locked,
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_remove_project(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    project_path: &str,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::remove_project(&state, instance_id, project_path).await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_update_managed_modrinth_version(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: String,
    version_id: String,
) -> Result<engine::install::InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::instance::update_managed_modrinth_version(
        &state,
        &instance_id,
        &version_id,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_repair_managed_modrinth(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<engine::install::InstallJobSnapshot> {
    let state = handle.get().await;
    Ok(engine::instance::repair_managed_modrinth(&state, instance_id).await?)
}

#[tauri::command]
pub async fn instance_export_mrpack(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    export_location: PathBuf,
    included_overrides: Vec<String>,
    excluded_overrides: Vec<String>,
    version_id: Option<String>,
    description: Option<String>,
    name: Option<String>,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::export_mrpack(
        &state,
        instance_id,
        export_location,
        included_overrides,
        excluded_overrides,
        version_id,
        description,
        name,
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_get_pack_export_candidates(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    parent: Option<SafeRelativeUtf8UnixPathBuf>,
) -> Result<Vec<engine::instance::PackExportCandidate>> {
    let state = handle.get().await;
    Ok(engine::instance::get_pack_export_candidates_for_parent(
        &state,
        instance_id,
        parent,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_run(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    server_address: Option<String>,
) -> Result<ProcessMetadata> {
    let state = handle.get().await;
    let quick_play = match server_address {
        Some(addr) => QuickPlayType::Server(ServerAddress::Unresolved(addr)),
        None => QuickPlayType::None,
    };
    Ok(engine::instance::run(&state, instance_id, quick_play).await?)
}

#[tauri::command]
pub async fn instance_kill(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::kill(&state, instance_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_edit(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    edit_instance: EditInstance,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::edit(&state, instance_id, edit_to_core(edit_instance)?)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_edit_icon(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    icon_path: Option<&Path>,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::edit_icon(&state, instance_id, icon_path).await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_cache_icon(
    handle: tauri::State<'_, EngineHandle>,
    icon_bytes: Vec<u8>,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::instance::cache_icon_bytes(&state, icon_bytes).await?)
}

#[tauri::command]
pub async fn instance_edit_generated_icon(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    config: engine::data::IconConfig,
    symbol_bytes: Vec<u8>,
    only_if_empty: Option<bool>,
) -> Result<Option<String>> {
    let state = handle.get().await;
    if only_if_empty.unwrap_or(false) {
        return Ok(engine::instance::edit_generated_icon_if_empty(
            &state,
            instance_id,
            config,
            symbol_bytes,
        )
        .await?);
    }

    Ok(Some(
        engine::instance::edit_generated_icon(
            &state,
            instance_id,
            config,
            symbol_bytes,
        )
        .await?,
    ))
}

#[tauri::command]
pub async fn instance_cache_generated_icon(
    handle: tauri::State<'_, EngineHandle>,
    config: engine::data::IconConfig,
    symbol_bytes: Vec<u8>,
    add_to_recents: bool,
) -> Result<String> {
    let state = handle.get().await;
    Ok(engine::instance::cache_generated_icon(
        &state,
        config,
        symbol_bytes,
        add_to_recents,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_share_can_current_user_use(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<bool> {
    let state = handle.get().await;
    Ok(engine::instance::can_active_user_use_shared_instances(&state).await?)
}

#[tauri::command]
pub async fn instance_share_get_users(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<engine::instance::SharedInstanceUsers> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_shared_instance_users(&state, instance_id)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_share_invite_users(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    user_ids: Vec<String>,
) -> Result<engine::instance::SharedInstanceUsers> {
    let state = handle.get().await;
    Ok(engine::instance::invite_shared_instance_users(
        &state,
        instance_id,
        user_ids,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_share_create_invite_link(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    max_age_seconds: Option<i32>,
    max_uses: Option<i32>,
    replace_invite_id: Option<String>,
) -> Result<engine::instance::SharedInstanceInviteLink> {
    let state = handle.get().await;
    Ok(engine::instance::create_shared_instance_invite_link(
        &state,
        instance_id,
        max_age_seconds,
        max_uses,
        replace_invite_id,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_share_get_invites(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<Vec<engine::instance::SharedInstanceInvite>> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_shared_instance_invites(&state, instance_id)
            .await?,
    )
}

#[tauri::command]
pub async fn instance_share_revoke_invite(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    invite_id: String,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::revoke_shared_instance_invite(
        &state,
        instance_id,
        invite_id,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_share_remove_users(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    user_ids: Vec<String>,
    has_pending_recipients: bool,
) -> Result<engine::instance::SharedInstanceUsers> {
    let state = handle.get().await;
    Ok(engine::instance::remove_shared_instance_users(
        &state,
        instance_id,
        user_ids,
        has_pending_recipients,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_share_get_publish_preview(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<Option<engine::instance::SharedInstancePublishPreview>> {
    let state = handle.get().await;
    Ok(engine::instance::get_shared_instance_publish_preview(
        &state,
        instance_id,
    )
    .await?)
}

#[tauri::command]
pub async fn instance_share_publish(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    file_paths: Vec<String>,
    removed_paths: Vec<String>,
    server_core_id: Option<String>,
    on_progress: tauri::ipc::Channel<(u64, u64)>,
) -> Result<SharedInstanceAttachment> {
    let state = handle.get().await;
    Ok(engine::instance::publish_shared_instance(
        &state,
        instance_id,
        file_paths,
        removed_paths,
        server_core_id,
        &|uploaded, total| {
            let _ = on_progress.send((uploaded, total));
        },
    )
    .await?
    .into())
}

#[tauri::command]
pub async fn instance_share_publish_support_details(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<Option<String>> {
    let state = handle.get().await;
    Ok(
        engine::instance::get_shared_instance_publish_support_details(
            &state,
            instance_id,
        )
        .await?,
    )
}

#[tauri::command]
pub async fn instance_share_unlink(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::unlink_shared_instance(&state, instance_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_share_unpublish(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::unpublish_shared_instance(&state, instance_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_share_ensure(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<SharedInstanceAttachment> {
    let state = handle.get().await;
    Ok(
        engine::instance::ensure_shared_instance(&state, instance_id)
            .await?
            .into(),
    )
}

#[tauri::command]
pub async fn instance_share_unpublish_remote(
    handle: tauri::State<'_, EngineHandle>,
    shared_instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::unpublish_remote_shared_instance(
        &state,
        shared_instance_id,
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_share_get_cached_library(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<engine::data::CachedSharedInstanceSummary>> {
    let state = handle.get().await;
    Ok(engine::instance::get_cached_shared_instance_library(&state).await?)
}

#[tauri::command]
pub async fn instance_share_cache_library(
    handle: tauri::State<'_, EngineHandle>,
    instances: Vec<engine::data::CachedSharedInstanceSummary>,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::cache_shared_instance_library(&state, instances).await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_get_hidden_ids(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<String>> {
    let state = handle.get().await;
    Ok(engine::instance::get_hidden_instance_ids(&state).await?)
}

#[tauri::command]
pub async fn instance_set_hidden(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    hidden: bool,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::set_instance_hidden(&state, instance_id, hidden).await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_share_dismiss_restored_notice(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::dismiss_shared_instance_restored_notice(
        &state,
        instance_id,
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn instance_share_leave_remote(
    handle: tauri::State<'_, EngineHandle>,
    shared_instance_id: &str,
) -> Result<()> {
    let state = handle.get().await;
    engine::instance::leave_remote_shared_instance(&state, shared_instance_id)
        .await?;
    Ok(())
}
