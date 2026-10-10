use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        backup_service::{
            create_backup, delete_backup, rename_backup, restore_backup,
            BackupError,
        },
        state::AppState,
    },
    presentation::{error::ApiError, instance_path::resolve_instance_id},
};

impl From<BackupError> for ApiError {
    fn from(e: BackupError) -> Self {
        match e {
            BackupError::NotFound => ApiError::NotFound("not found".into()),
            BackupError::Locked => {
                ApiError::Conflict("backup is locked".into())
            }
            BackupError::MustBeOffline => {
                ApiError::Conflict("instance must be offline to restore".into())
            }
            BackupError::RconRequiredForHotBackup => {
                ApiError::Conflict("rcon_required_for_hot_backup".into())
            }
            BackupError::PathTraversal => {
                ApiError::BadRequest("path traversal rejected".into())
            }
            BackupError::Rcon(message) => ApiError::ServiceUnavailable(message),
            e => ApiError::Internal(e.to_string()),
        }
    }
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct RenameBody {
    pub name: String,
}

/// POST /instances/:id/backups
pub async fn create_handler(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_backup_instance_id(&state, &id).await?;
    let backup = create_backup(&state, &instance_id, "manual", body.name)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(backup_json(backup)))
}

fn backup_json(b: crate::application::backup_service::BackupRecord) -> Value {
    json!({
        "id": b.id,
        "name": b.name,
        "size_bytes": b.size_bytes,
        "locked": b.locked,
        "automated": b.trigger == "scheduled",
        "hot": b.hot,
        "consistency": b.consistency,
        "trigger": b.trigger,
        "status": "done",
        "created_at": b.created_at,
    })
}

/// DELETE /instances/:id/backups/:bid
pub async fn delete_handler(
    Path((id, bid)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_backup_instance_id(&state, &id).await?;
    delete_backup(&state, &instance_id, &bid)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/backups/:bid/restore
pub async fn restore_handler(
    Path((id, bid)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_backup_instance_id(&state, &id).await?;
    restore_backup(&state, &instance_id, &bid)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// PATCH /instances/:id/backups/:bid — rename a backup.
pub async fn rename_handler(
    Path((id, bid)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<RenameBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_backup_instance_id(&state, &id).await?;
    rename_backup(&state, &instance_id, &bid, body.name)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

async fn resolve_backup_instance_id(
    state: &Arc<AppState>,
    path: &str,
) -> Result<String, ApiError> {
    Ok(resolve_instance_id(state, path).await?.to_string())
}
