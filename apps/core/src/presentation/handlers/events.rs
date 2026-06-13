use std::sync::Arc;

use axum::{
    extract::State,
    response::sse::{Event as SseEvent, KeepAlive, Sse},
};
use futures::StreamExt;
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;

use crate::{
    application::state::AppState,
    domain::event::Event,
    presentation::{
        authz::require_core_member, error::ApiError, extractors::AuthUser,
    },
};

use super::instances::record_list_item;

/// GET /events — live stream of Core state changes for UI state managers.
pub async fn stream_events(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<
    Sse<
        impl futures::Stream<Item = Result<SseEvent, std::convert::Infallible>>,
    >,
    ApiError,
> {
    require_core_member(&state, &claims.sub).await?;
    let stream = BroadcastStream::new(state.broadcaster.subscribe())
        .filter_map(|msg| async move {
            let event = match msg.ok()? {
                Event::InstanceCreated { instance } => json!({
                    "type": "instance_created",
                    "instance": record_list_item(&instance),
                }),
                Event::InstanceUpdated { instance } => json!({
                    "type": "instance_updated",
                    "instance": record_list_item(&instance),
                }),
                Event::InstanceDeleted { instance_id } => json!({
                    "type": "instance_deleted",
                    "instance_id": instance_id.to_string(),
                }),
                Event::StatusChanged {
                    instance_id,
                    status,
                } => json!({
                    "type": "status_changed",
                    "instance_id": instance_id.to_string(),
                    "status": status.to_string(),
                }),
                Event::CreationProgress {
                    instance_id,
                    progress,
                    message,
                } => json!({
                    "type": "creation_progress",
                    "instance_id": instance_id.to_string(),
                    "progress": progress,
                    "message": message,
                }),
                Event::InstallStatusChanged {
                    instance_id,
                    install_status,
                    message,
                } => json!({
                    "type": "install_status_changed",
                    "instance_id": instance_id.to_string(),
                    "install_status": install_status.to_string(),
                    "message": message,
                }),
                Event::InstallationStatusChanged {
                    installation_id,
                    status,
                    message,
                } => json!({
                    "type": "installation_status_changed",
                    "installation_id": installation_id,
                    "status": status.to_string(),
                    "message": message,
                }),
                Event::FsChanged {
                    instance_id,
                    operation,
                    path,
                } => json!({
                    "type": "fs_changed",
                    "instance_id": instance_id.to_string(),
                    "operation": operation,
                    "path": path,
                }),
                Event::SyncProfileUpdated {
                    profile_id,
                    snapshot_id,
                    instance_id,
                } => json!({
                    "type": "sync_profile_updated",
                    "profile_id": profile_id,
                    "snapshot_id": snapshot_id,
                    "instance_id": instance_id,
                }),
                Event::SyncEventStatusChanged {
                    profile_id,
                    event_id,
                    status,
                    message,
                } => json!({
                    "type": "sync_event_status_changed",
                    "profile_id": profile_id,
                    "event_id": event_id,
                    "status": status,
                    "message": message,
                }),
                Event::InstanceOutput { .. } | Event::MacroOutput { .. } => {
                    return None
                }
            };

            Some(Ok(SseEvent::default().data(event.to_string())))
        });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
