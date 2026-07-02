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
            create_backup, delete_backup, delete_many_backups,
            get_backup_schedule, list_backups, lock_backup, rename_backup,
            restore_backup, set_backup_schedule, BackupError,
        },
        state::AppState,
    },
    presentation::{
        error::ApiError, extractors::AuthUser,
        instance_path::resolve_authorized_instance_id,
    },
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
            BackupError::InvalidSchedule(message) => {
                ApiError::BadRequest(message)
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
pub struct DeleteManyBody {
    pub ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct LockBody {
    pub locked: bool,
}

#[derive(Deserialize)]
pub struct RenameBody {
    pub name: String,
}

#[derive(Deserialize)]
pub struct ScheduleBody {
    pub enabled: bool,
    pub cron: String,
    pub retain_count: i64,
}

/// GET /instances/:id/backups
pub async fn list_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_backup_instance_id(&state, &claims.sub, &id).await?;
    let rows = list_backups(&state, &instance_id)
        .await
        .map_err(ApiError::from)?;
    let backups: Vec<Value> = rows.into_iter().map(backup_json).collect();
    let active_operations: Vec<Value> = vec![];
    Ok(Json(
        json!({ "backups": backups, "active_operations": active_operations }),
    ))
}

/// POST /instances/:id/backups
pub async fn create_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_backup_instance_id(&state, &claims.sub, &id).await?;
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
    AuthUser(claims): AuthUser,
    Path((id, bid)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_backup_instance_id(&state, &claims.sub, &id).await?;
    delete_backup(&state, &instance_id, &bid)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/backups/delete-many
pub async fn delete_many_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<DeleteManyBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_backup_instance_id(&state, &claims.sub, &id).await?;
    let deleted = delete_many_backups(&state, &instance_id, &body.ids)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "deleted": deleted })))
}

/// PATCH /instances/:id/backups/:bid/lock
pub async fn lock_handler(
    AuthUser(claims): AuthUser,
    Path((id, bid)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<LockBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_backup_instance_id(&state, &claims.sub, &id).await?;
    lock_backup(&state, &instance_id, &bid, body.locked)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/backups/:bid/restore
pub async fn restore_handler(
    AuthUser(claims): AuthUser,
    Path((id, bid)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_backup_instance_id(&state, &claims.sub, &id).await?;
    restore_backup(&state, &instance_id, &bid)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// GET /instances/:id/backups/schedule
pub async fn get_schedule_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_backup_instance_id(&state, &claims.sub, &id).await?;
    let schedule = get_backup_schedule(&state, &instance_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!(schedule)))
}

/// PUT /instances/:id/backups/schedule
pub async fn set_schedule_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ScheduleBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_backup_instance_id(&state, &claims.sub, &id).await?;
    set_backup_schedule(
        &state,
        &instance_id,
        body.enabled,
        &body.cron,
        body.retain_count,
    )
    .await
    .map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// PATCH /instances/:id/backups/:bid — rename a backup.
pub async fn rename_handler(
    AuthUser(claims): AuthUser,
    Path((id, bid)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<RenameBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_backup_instance_id(&state, &claims.sub, &id).await?;
    rename_backup(&state, &instance_id, &bid, body.name)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

async fn resolve_backup_instance_id(
    state: &Arc<AppState>,
    user_id: &str,
    path: &str,
) -> Result<String, ApiError> {
    Ok(
        resolve_authorized_instance_id(state, user_id, path, "server:backups")
            .await?
            .to_string(),
    )
}
