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
			create_backup, delete_backup, delete_many_backups, get_backup_schedule, list_backups,
			lock_backup, rename_backup, restore_backup, set_backup_schedule, BackupError,
		},
		state::AppState,
	},
	domain::instance::InstanceId,
	presentation::{error::ApiError, extractors::AuthUser},
};

fn validate_id(id: &str) -> Result<(), ApiError> {
	id.parse::<InstanceId>()
		.map(|_| ())
		.map_err(|_| ApiError::BadRequest("invalid instance id".into()))
}

impl From<BackupError> for ApiError {
	fn from(e: BackupError) -> Self {
		match e {
			BackupError::NotFound => ApiError::NotFound("not found".into()),
			BackupError::Locked => ApiError::Conflict("backup is locked".into()),
			BackupError::MustBeOffline => ApiError::Conflict("instance must be offline to restore".into()),
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
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	let backups = list_backups(&state, &id).await.map_err(ApiError::from)?;
	Ok(Json(json!({ "backups": backups })))
}

/// POST /instances/:id/backups
pub async fn create_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<CreateBody>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	let backup = create_backup(&state, &id, "manual", body.name).await.map_err(ApiError::from)?;
	Ok(Json(json!(backup)))
}

/// DELETE /instances/:id/backups/:bid
pub async fn delete_handler(
	_auth: AuthUser,
	Path((id, bid)): Path<(String, String)>,
	State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	delete_backup(&state, &id, &bid).await.map_err(ApiError::from)?;
	Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/backups/delete-many
pub async fn delete_many_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<DeleteManyBody>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	let deleted = delete_many_backups(&state, &id, &body.ids).await.map_err(ApiError::from)?;
	Ok(Json(json!({ "deleted": deleted })))
}

/// PATCH /instances/:id/backups/:bid/lock
pub async fn lock_handler(
	_auth: AuthUser,
	Path((id, bid)): Path<(String, String)>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<LockBody>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	lock_backup(&state, &id, &bid, body.locked).await.map_err(ApiError::from)?;
	Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/backups/:bid/restore
pub async fn restore_handler(
	_auth: AuthUser,
	Path((id, bid)): Path<(String, String)>,
	State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	restore_backup(&state, &id, &bid).await.map_err(ApiError::from)?;
	Ok(Json(json!({ "ok": true })))
}

/// GET /instances/:id/backups/schedule
pub async fn get_schedule_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	let schedule = get_backup_schedule(&state, &id).await.map_err(ApiError::from)?;
	Ok(Json(json!(schedule)))
}

/// PUT /instances/:id/backups/schedule
pub async fn set_schedule_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<ScheduleBody>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	set_backup_schedule(&state, &id, body.enabled, &body.cron, body.retain_count)
		.await
		.map_err(ApiError::from)?;
	Ok(Json(json!({ "ok": true })))
}

/// PATCH /instances/:id/backups/:bid — rename a backup.
pub async fn rename_handler(
	_auth: AuthUser,
	Path((id, bid)): Path<(String, String)>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<RenameBody>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	rename_backup(&state, &id, &bid, body.name).await.map_err(ApiError::from)?;
	Ok(Json(json!({ "ok": true })))
}
