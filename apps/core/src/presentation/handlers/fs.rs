use std::sync::Arc;

use axum::{
	extract::{Multipart, Path, Query, State},
	http::{header, StatusCode},
	response::{IntoResponse, Response},
	Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
	application::{
		fs_service::{delete_entry, download_file, list_directory, upload_file, FsError},
		state::AppState,
	},
	domain::instance::InstanceId,
	presentation::{error::ApiError, extractors::AuthUser},
};

#[derive(Deserialize)]
pub struct ListQuery {
	pub path: Option<String>,
	pub page: Option<usize>,
	pub page_size: Option<usize>,
}

#[derive(Deserialize)]
pub struct DownloadQuery {
	pub path: String,
}

#[derive(Deserialize)]
pub struct UploadQuery {
	pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct DeleteBody {
	pub path: String,
	pub recursive: Option<bool>,
}

fn validate_id(id: &str) -> Result<(), ApiError> {
	id.parse::<InstanceId>()
		.map(|_| ())
		.map_err(|_| ApiError::BadRequest("invalid instance id".into()))
}

impl From<FsError> for ApiError {
	fn from(e: FsError) -> Self {
		match e {
			FsError::NotFound => ApiError::NotFound("instance not found".into()),
			FsError::PathTraversal => ApiError::Unauthorized("path traversal rejected".into()),
			e => ApiError::Internal(e.to_string()),
		}
	}
}

/// GET /instances/:id/fs — list directory contents (paginated).
pub async fn list_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	Query(q): Query<ListQuery>,
	State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	let path = q.path.as_deref().unwrap_or("");
	let listing = list_directory(&state, &id, path, q.page.unwrap_or(0), q.page_size.unwrap_or(50))
		.await
		.map_err(ApiError::from)?;
	Ok(Json(json!(listing)))
}

/// GET /instances/:id/fs/download — download a single file.
pub async fn download_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	Query(q): Query<DownloadQuery>,
	State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
	validate_id(&id)?;
	let (bytes, filename) = download_file(&state, &id, &q.path).await.map_err(ApiError::from)?;
	let disposition = format!("attachment; filename=\"{filename}\"");
	Ok((
		StatusCode::OK,
		[
			(header::CONTENT_TYPE, "application/octet-stream"),
			(header::CONTENT_DISPOSITION, disposition.as_str()),
		],
		bytes,
	)
		.into_response())
}

/// DELETE /instances/:id/fs — delete a file or directory.
pub async fn delete_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<DeleteBody>,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	delete_entry(&state, &id, &body.path, body.recursive.unwrap_or(false))
		.await
		.map_err(ApiError::from)?;
	Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/fs/upload — upload a file into the instance directory.
pub async fn upload_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	Query(q): Query<UploadQuery>,
	State(state): State<Arc<AppState>>,
	mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
	validate_id(&id)?;
	let target_dir = q.path.as_deref().unwrap_or("");

	while let Some(field) = multipart
		.next_field()
		.await
		.map_err(|e| ApiError::BadRequest(e.to_string()))?
	{
		let filename = field.file_name().unwrap_or("file").to_string();
		let data = field
			.bytes()
			.await
			.map_err(|e| ApiError::BadRequest(e.to_string()))?;
		upload_file(&state, &id, target_dir, &filename, data)
			.await
			.map_err(ApiError::from)?;
		return Ok(Json(json!({ "ok": true, "filename": filename })));
	}
	Err(ApiError::BadRequest("no file provided".into()))
}
