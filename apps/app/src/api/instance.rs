use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::endpoints::instances::SharedInstanceAttachment;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("instance")
        .invoke_handler(tauri::generate_handler![
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
            instance_share_dismiss_restored_notice,
        ])
        .build()
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
