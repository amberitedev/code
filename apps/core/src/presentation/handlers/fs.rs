use std::{sync::Arc, time::Instant};

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        fs_service::{
            copy_files, create_dir, create_file, delete_entry, download_file,
            get_download_url, list_directory, move_entry, read_file,
            search_files, unzip_file, upload_file, write_file, zip_files,
            FsError, UnzipOption,
        },
        state::AppState,
	},
	presentation::{
		error::ApiError, extractors::AuthUser,
		instance_path::resolve_authorized_instance_id,
	},
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

#[derive(Deserialize)]
pub struct PathQuery {
    pub path: String,
}

#[derive(Deserialize)]
pub struct MoveQuery {
    pub from: String,
    pub to: String,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub path: Option<String>,
    pub query: String,
    pub recursive: Option<bool>,
}

#[derive(Deserialize)]
pub struct UnzipBody {
    pub option: Option<UnzipOption>,
}

#[derive(Deserialize)]
pub struct ZipBody {
    pub sources: Vec<String>,
    pub dest: String,
}

#[derive(Deserialize)]
pub struct CopyBody {
    pub sources: Vec<String>,
    pub dest: String,
}

impl From<FsError> for ApiError {
    fn from(e: FsError) -> Self {
        match e {
            FsError::NotFound => {
                ApiError::NotFound("instance not found".into())
            }
            FsError::PathTraversal => {
                ApiError::Unauthorized("path traversal rejected".into())
            }
            FsError::NotAFile => {
                ApiError::BadRequest("path is a directory, not a file".into())
            }
            e => ApiError::Internal(e.to_string()),
        }
    }
}

/// GET /instances/:id/fs — list directory contents (paginated).
pub async fn list_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<ListQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	let path = q.path.as_deref().unwrap_or("");
	let listing = list_directory(
		&state,
		&instance_id,
		path,
		q.page.unwrap_or(0),
        q.page_size.unwrap_or(50),
    )
    .await
    .map_err(ApiError::from)?;
    Ok(Json(json!(listing)))
}

/// GET /instances/:id/fs/download — download a single file.
pub async fn download_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<DownloadQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	let (file, filename) = download_file(&state, &instance_id, &q.path)
		.await
		.map_err(ApiError::from)?;
    let len = file
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .len();
    let stream = tokio_util::io::ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);
    let disposition = format!("attachment; filename=\"{filename}\"");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, len)
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(body)
        .map_err(|e| ApiError::Internal(e.to_string()))?)
}

/// DELETE /instances/:id/fs — delete a file or directory.
pub async fn delete_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<DeleteBody>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	delete_entry(
		&state,
		&instance_id,
		&body.path,
		body.recursive.unwrap_or(false),
	)
	.await
	.map_err(ApiError::from)?;
	Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/fs/upload — upload a file into the instance directory.
pub async fn upload_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<UploadQuery>,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
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
		upload_file(&state, &instance_id, target_dir, &filename, data)
			.await
			.map_err(ApiError::from)?;
        return Ok(Json(json!({ "ok": true, "filename": filename })));
    }
    Err(ApiError::BadRequest("no file provided".into()))
}

/// GET /instances/:id/fs/read — read a file as raw bytes.
pub async fn read_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	let file = read_file(&state, &instance_id, &q.path)
		.await
		.map_err(ApiError::from)?;
    let len = file
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .len();
    let stream = tokio_util::io::ReaderStream::new(file);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, len)
        .body(axum::body::Body::from_stream(stream))
        .map_err(|e| ApiError::Internal(e.to_string()))?)
}

/// PUT /instances/:id/fs/write — write raw bytes to a file (creates or overwrites).
pub async fn write_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	write_file(&state, &instance_id, &q.path, body)
		.await
		.map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/fs/create — create an empty file.
pub async fn create_file_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	create_file(&state, &instance_id, &q.path)
		.await
		.map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/fs/mkdir — create a directory (and parents).
pub async fn mkdir_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	create_dir(&state, &instance_id, &q.path)
		.await
		.map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// PUT /instances/:id/fs/move — move or rename a file/directory.
pub async fn move_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<MoveQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	move_entry(&state, &instance_id, &q.from, &q.to)
		.await
		.map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/fs/unzip — extract an archive.
pub async fn unzip_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UnzipBody>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	unzip_file(
		&state,
		&instance_id,
		&q.path,
		body.option.unwrap_or(UnzipOption::Normal),
    )
    .await
    .map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/fs/zip — zip a list of files.
pub async fn zip_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ZipBody>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	zip_files(&state, &instance_id, body.sources, &body.dest)
		.await
		.map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/fs/copy — copy files to a destination directory.
pub async fn copy_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CopyBody>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	copy_files(&state, &instance_id, body.sources, &body.dest)
		.await
		.map_err(ApiError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// GET /instances/:id/fs/url — issue a one-time download token.
pub async fn url_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	let resp = get_download_url(&state, &instance_id, &q.path)
		.await
		.map_err(ApiError::from)?;
    Ok(Json(json!(resp)))
}

/// GET /instances/:id/fs/search — search files by name.
pub async fn search_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<SearchQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	let instance_id = resolve_fs_instance_id(&state, &claims.sub, &id)
		.await?;
	let base = q.path.as_deref().unwrap_or("");
	let results =
		search_files(
			&state,
			&instance_id,
			base,
			&q.query,
			q.recursive.unwrap_or(false),
		)
		.await
		.map_err(ApiError::from)?;
	Ok(Json(json!(results)))
}

async fn resolve_fs_instance_id(
	state: &Arc<AppState>,
	user_id: &str,
	path: &str,
) -> Result<String, ApiError> {
	Ok(resolve_authorized_instance_id(state, user_id, path, "server:files")
		.await?
		.to_string())
}

/// GET /fs/file/:key — stream a file using a one-time download token (no auth — token IS the credential).
pub async fn download_by_key_handler(
    Path(key): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
    let token = state
        .fs_download_tokens
        .remove(&key)
        .map(|(_, t)| t)
        .ok_or_else(|| {
            ApiError::NotFound("download token not found or expired".into())
        })?;

    if token.expires_at < Instant::now() {
        return Err(ApiError::Unauthorized("download token expired".into()));
    }

    let path = token.path;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let len = file
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .len();
    let stream = tokio_util::io::ReaderStream::new(file);
    let disposition = format!("attachment; filename=\"{filename}\"");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, len)
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(axum::body::Body::from_stream(stream))
        .map_err(|e| ApiError::Internal(e.to_string()))?)
}
