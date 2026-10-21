//! A server's files, under `/servers/{id}`: `modrinth/v0/fs/` list, download, create, update, move,
//! and delete; `v1/worlds/{wid}/files/stat` and `.../files/zip`; `v1/fs/unarchive`;
//! `v1/fs/ops/{action}`; and the upload sessions at `v1/worlds/{wid}/{scope}/upload-session`
//! (create, get, files, finalize, cancel). Mirrors `Kyros.Files` and `Kyros.UploadSessions` in
//! `packages/api-client/src/modules/kyros/types.ts`. Also `GET /modrinth/v0/servers/{id}/fs`,
//! where the App learns the base of these routes.
//!
//! Editors and the owner only: the files hold the world and every setting. Every path goes
//! through `content::folder`, so nothing reaches outside the server folder, links included.
//! Uploads and unpacking never replace server-owned files (see `folder::is_server_owned`);
//! `fs/update` is an explicit edit and may change them. `fs/copy` and the v0 `fs/zip` have no
//! caller in the App and are not served.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use super::{
    AppState,
    activity::Action,
    content::{MAX_UPLOAD, stage_field, world},
    error::{ApiError, ErrorCode},
    servers::JwtAuth,
};
use crate::{
    access::{Caller, Level, activity, now},
    content::{
        Busy, addons,
        archive::{self, ArchiveError},
        folder::{self, Folder},
        pack,
    },
};

/// Query of GET `fs/download` and PUT `fs/update`. Mirrors the `path` param of
/// `KyrosFilesV0Module.downloadFile` and `.updateFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct PathQuery {
    pub path: String,
}

/// Query of GET `fs/list`. Mirrors the params of `KyrosFilesV0Module.listDirectory`.
#[derive(Debug, Clone, Deserialize)]
pub struct ListQuery {
    pub path: String,
    pub page: u32,
    pub page_size: u32,
}

/// Mirrors the `type` union of Kyros.Files.v0.DirectoryItem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DirectoryItemType {
    File,
    Directory,
    Symlink,
}

/// One entry of a directory listing. Times are Unix seconds. Mirrors Kyros.Files.v0.DirectoryItem.
#[derive(Debug, Clone, Serialize)]
pub struct DirectoryItem {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: DirectoryItemType,
    pub path: String,
    pub modified: i64,
    pub created: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// Response of GET `fs/list`. `current` is the page returned. Mirrors
/// Kyros.Files.v0.DirectoryResponse.
#[derive(Debug, Clone, Serialize)]
pub struct DirectoryResponse {
    pub items: Vec<DirectoryItem>,
    pub total: u64,
    pub current: u32,
}

/// Mirrors the `type` param of `KyrosFilesV0Module.createFileOrFolder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CreateKind {
    File,
    Directory,
}

/// Query of POST `fs/create`. A file's contents are the request body. Mirrors the params of
/// `KyrosFilesV0Module.createFileOrFolder` and `.uploadFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateQuery {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: CreateKind,
}

/// Query of DELETE `fs/delete`. Mirrors the params of `KyrosFilesV0Module.deleteFileOrFolder`.
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteQuery {
    pub path: String,
    pub recursive: bool,
}

/// Body of POST `fs/move`. Mirrors the body of `KyrosFilesV0Module.moveFileOrFolder`.
#[derive(Debug, Clone, Deserialize)]
pub struct MoveRequest {
    pub source: String,
    pub destination: String,
}

/// Query of POST `v1/fs/unarchive`: extract `src` into `trg`. `dry` only reports conflicts.
/// Mirrors the params of `KyrosFilesV0Module.extractFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct UnarchiveQuery {
    pub src: String,
    pub trg: String,
    #[serde(rename = "override")]
    pub overwrite: bool,
    pub dry: bool,
}

/// Response of POST `v1/fs/unarchive`. Mirrors Kyros.Files.v0.ExtractResult.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractResult {
    pub modpack_name: Option<String>,
    pub conflicting_files: Vec<String>,
}

/// The `{action}` of POST `v1/fs/ops/{action}`. Mirrors the `action` of
/// `KyrosFilesV0Module.modifyOperation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationAction {
    Dismiss,
    Cancel,
}

/// Query of POST `v1/fs/ops/{action}`. Mirrors the `id` param of
/// `KyrosFilesV0Module.modifyOperation`.
#[derive(Debug, Clone, Deserialize)]
pub struct OperationQuery {
    #[allow(dead_code)] // There are never operations to name.
    pub id: String,
}

/// Body of POST `v1/worlds/{wid}/files/stat`. Mirrors Kyros.Files.v1.FileStatRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct FileStatRequest {
    pub path: String,
}

/// Mirrors Kyros.Files.v1.DescendantType.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DescendantType {
    Regular,
    Directory,
    Symlink,
    Other,
}

/// Response of POST `v1/worlds/{wid}/files/stat`. Mirrors Kyros.Files.v1.FileStatResponse.
#[derive(Debug, Clone, Serialize)]
pub struct FileStatResponse {
    pub name: String,
    pub full_path: String,
    pub size_bytes: u64,
    #[serde(rename = "type")]
    pub kind: DescendantType,
    pub mtime: DateTime<Utc>,
    pub ctime: DateTime<Utc>,
}

/// Body of POST `v1/worlds/{wid}/files/zip`: zip a directory, or some paths inside `parent` into
/// `target`. Mirrors Kyros.Files.v1.ZipRequest.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "target_type")]
pub enum ZipRequest {
    Directory {
        path: String,
    },
    ManyPaths {
        parent: String,
        include: Vec<String>,
        target: String,
    },
}

/// One record of the `application/json-seq` response of POST `v1/worlds/{wid}/files/zip`. Mirrors
/// Kyros.Files.v1.ZipProgress.
#[derive(Debug, Clone, Serialize)]
pub struct ZipProgress {
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The `{scope}` of the upload session routes: content goes through the content model, files
/// land where they are put. Mirrors Kyros.UploadSessions.v1.Scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadScope {
    Content,
    Files,
}

/// Mirrors Kyros.UploadSessions.v1.UploadSessionStatus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadSessionStatus {
    Active,
    Uploading,
    Finalizing,
    Cancelled,
    Finalized,
    #[allow(dead_code)] // An expired session is gone rather than shown.
    Expired,
}

/// Response of POST `.../upload-session` (create), POST `.../{upload}/files`, POST
/// `.../{upload}/finalize`, and DELETE `.../{upload}` (cancel). Times are Unix seconds.
/// Mirrors Kyros.UploadSessions.v1.UploadSessionResponse.
#[derive(Debug, Clone, Serialize)]
pub struct UploadSessionResponse {
    pub upload_id: String,
    pub status: UploadSessionStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_upload_at: Option<i64>,
    pub expires_at: i64,
    pub entry_count: u32,
    pub uploaded_byte_count: u64,
}

/// Response of GET `.../upload-session`: the caller's open session, if any. Mirrors
/// Kyros.UploadSessions.v1.GetUploadSessionResponse.
#[derive(Debug, Clone, Serialize)]
pub struct GetUploadSessionResponse {
    pub session: Option<UploadSessionResponse>,
}

/// How long an upload session lives without a request.
const SESSION_IDLE_S: i64 = 3600;

/// One upload session: files staged in its own folder, out of the server folder, until it is
/// finalized.
#[derive(Debug, Clone)]
struct Session {
    server_id: String,
    scope: UploadScope,
    /// Who opened it; `None` for the local CLI.
    user_id: Option<String>,
    status: UploadSessionStatus,
    created_at: i64,
    updated_at: i64,
    last_upload_at: Option<i64>,
    /// Where its files wait. Removed whenever the session ends.
    dir: PathBuf,
    /// The name each file was sent with, and where it waits.
    files: Vec<(String, PathBuf)>,
    bytes: u64,
}

impl Session {
    fn expires_at(&self) -> i64 {
        self.updated_at + SESSION_IDLE_S
    }

    fn response(&self, upload_id: &str) -> UploadSessionResponse {
        UploadSessionResponse {
            upload_id: upload_id.to_owned(),
            status: self.status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            last_upload_at: self.last_upload_at,
            expires_at: self.expires_at(),
            entry_count: self.files.len() as u32,
            uploaded_byte_count: self.bytes,
        }
    }

    fn open(&self) -> bool {
        matches!(
            self.status,
            UploadSessionStatus::Active | UploadSessionStatus::Uploading
        )
    }

    fn belongs_to(
        &self,
        caller: &Caller,
        server_id: &str,
        scope: UploadScope,
    ) -> bool {
        self.server_id == server_id
            && self.scope == scope
            && self.user_id.as_deref() == caller.user_id()
    }

    /// End the session: it is over and holds no files.
    fn end(&mut self, status: UploadSessionStatus) {
        self.status = status;
        self.updated_at = now();
        self.files.clear();
    }
}

/// Every upload session, by id. Sessions live in memory: a Core restart ends them, and the next
/// one clears `staging/`.
#[derive(Default)]
pub struct Uploads {
    sessions: Mutex<HashMap<String, Session>>,
}

impl Uploads {
    /// The sessions, after dropping those an hour without a request, whatever their state, and
    /// removing what they staged.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Session>> {
        let mut sessions = self.sessions.lock().expect("not poisoned");
        let now = now();
        sessions.retain(|_, session| {
            let live = session.expires_at() > now;
            if !live {
                let _ = std::fs::remove_dir_all(&session.dir);
            }
            live
        });
        sessions
    }

    /// Run `change` on the caller's session `upload_id`, under the lock, if it is still open.
    fn change<T>(
        &self,
        caller: &Caller,
        server_id: &str,
        scope: UploadScope,
        upload_id: &str,
        change: impl FnOnce(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        let mut sessions = self.lock();
        let session = sessions
            .get_mut(upload_id)
            .filter(|session| session.belongs_to(caller, server_id, scope))
            .ok_or_else(|| {
                ApiError::new(ErrorCode::NotFound, "There is no such upload.")
            })?;
        if !session.open() {
            return Err(over());
        }
        change(session)
    }
}

fn over() -> ApiError {
    ApiError::new(ErrorCode::Conflict, "This upload is over.")
}

fn not_found(path: &str) -> ApiError {
    ApiError::new(ErrorCode::NotFound, format!("There is no {path}."))
}

fn folder(state: &AppState, id: &str) -> Result<Arc<Folder>, ApiError> {
    Ok(Arc::new(Folder::open(&state.servers.paths().server(id))?))
}

/// The folder, taken for a change: the same lock installs hold, so an edit never lands between
/// an install's plan and its writes. Hold the lock until the change is done.
fn changing(
    state: &AppState,
    id: &str,
) -> Result<(Arc<Folder>, Busy), ApiError> {
    let busy = state.content.busy(id)?;
    Ok((folder(state, id)?, busy))
}

/// A path the App sent, checked. `None` is the server folder itself.
fn api_path(path: &str) -> Result<Option<String>, ApiError> {
    folder::from_api(path).map_err(|error| {
        ApiError::new(ErrorCode::InvalidRequest, error.to_string())
    })
}

/// A path that must name something inside the folder, not the folder itself.
fn inner_path(path: &str) -> Result<String, ApiError> {
    api_path(path)?.ok_or_else(|| {
        ApiError::new(ErrorCode::InvalidRequest, "Name a file or folder.")
    })
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(work).await.map_err(|error| {
        ApiError::new(ErrorCode::Internal, error.to_string())
    })?
}

fn unix(time: std::io::Result<cap_std::time::SystemTime>) -> i64 {
    time.ok()
        .map(cap_std::time::SystemTime::into_std)
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs() as i64)
}

/// `GET /modrinth/v0/servers/{id}/fs`: where the file routes are. They check the session
/// themselves, so the token is empty.
pub async fn auth(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<JwtAuth>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let host = headers
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .unwrap_or("localhost");
    Ok(Json(JwtAuth {
        url: format!("{host}/servers/{id}"),
        token: String::new(),
    }))
}

/// `GET fs/list`: one page of a folder, folders first.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<DirectoryResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let path = api_path(&query.path)?;
    let folder = folder(&state, &id)?;
    let mut items = blocking(move || {
        let dir = folder
            .open_dir(path.as_deref())
            .map_err(|_| not_found(path.as_deref().unwrap_or("folder")))?;
        let mut items = Vec::new();
        for entry in dir.entries()?.filter_map(Result::ok) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            let kind = if meta.file_type().is_symlink() {
                DirectoryItemType::Symlink
            } else if meta.is_dir() {
                DirectoryItemType::Directory
            } else {
                DirectoryItemType::File
            };
            let count = (kind == DirectoryItemType::Directory)
                .then(|| {
                    entry
                        .open_dir()
                        .ok()?
                        .entries()
                        .ok()
                        .map(|entries| entries.count() as u64)
                })
                .flatten();
            items.push(DirectoryItem {
                path: format!(
                    "/{}",
                    path.as_ref()
                        .map_or(name.clone(), |path| format!("{path}/{name}"))
                ),
                name,
                kind,
                modified: unix(meta.modified()),
                created: unix(meta.created()),
                size: (kind == DirectoryItemType::File).then(|| meta.len()),
                count,
                target: None,
            });
        }
        Ok(items)
    })
    .await?;
    items.sort_by(|a, b| {
        (a.kind != DirectoryItemType::Directory)
            .cmp(&(b.kind != DirectoryItemType::Directory))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let total = items.len() as u64;
    let size = query.page_size.clamp(1, 1000) as usize;
    let page = query.page.max(1);
    let items = items
        .into_iter()
        .skip((page as usize - 1) * size)
        .take(size)
        .collect();
    Ok(Json(DirectoryResponse {
        items,
        total,
        current: page,
    }))
}

/// `GET fs/download`: a file, streamed.
pub async fn download(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<PathQuery>,
) -> Result<Response, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let path = inner_path(&query.path)?;
    let folder = folder(&state, &id)?;
    let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
    let file = blocking(move || {
        let file = folder
            .open_file(&path)
            .ok()
            .flatten()
            .ok_or_else(|| not_found(&path))?;
        Ok(file.into_std())
    })
    .await?;
    let length = file.metadata().map(|meta| meta.len()).unwrap_or_default();
    let stream =
        tokio_util::io::ReaderStream::new(tokio::fs::File::from_std(file));
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (header::CONTENT_LENGTH, length.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", name.replace('"', "")),
            ),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

/// Stream a request body into a staged file, at most [`MAX_UPLOAD`] bytes.
async fn stage_body(
    state: &AppState,
    id: &str,
    body: Body,
) -> Result<PathBuf, ApiError> {
    let staged = state
        .servers
        .paths()
        .staging(id)
        .join(format!("body-{}", uuid::Uuid::new_v4()));
    if let Some(parent) = staged.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let result = async {
        let mut file = tokio::fs::File::create(&staged).await?;
        let mut stream = body.into_data_stream();
        let mut written = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| {
                ApiError::new(ErrorCode::InvalidRequest, error.to_string())
            })?;
            written += chunk.len() as u64;
            if written > MAX_UPLOAD {
                return Err(ApiError::new(
                    ErrorCode::InvalidRequest,
                    "Uploads are at most 4 GB.",
                ));
            }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&staged).await;
    }
    result.map(|()| staged)
}

/// Write a staged file into the folder, then remove it.
async fn place(
    folder: Arc<Folder>,
    path: String,
    staged: PathBuf,
) -> Result<(), ApiError> {
    let result = blocking({
        let staged = staged.clone();
        move || {
            folder.write(&path, &mut std::fs::File::open(&staged)?)?;
            Ok(())
        }
    })
    .await;
    let _ = tokio::fs::remove_file(&staged).await;
    result
}

/// Take the folder for a change and write a staged file into it; the staged file is removed
/// either way. `owned` lets it replace a server-owned file.
async fn place_changing(
    state: &AppState,
    id: &str,
    path: String,
    staged: PathBuf,
    owned: bool,
) -> Result<(), ApiError> {
    let taken = changing(state, id).and_then(|(folder, busy)| {
        if !owned {
            refuse_owned(&folder, &path)?;
        }
        Ok((folder, busy))
    });
    match taken {
        Ok((folder, _busy)) => place(folder, path, staged).await,
        Err(error) => {
            let _ = tokio::fs::remove_file(&staged).await;
            Err(error)
        }
    }
}

/// Refuse to replace a server-owned file that exists.
fn refuse_owned(folder: &Folder, path: &str) -> Result<(), ApiError> {
    if folder::is_server_owned(path, &folder.level_name())
        && folder.exists(path)
    {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            format!(
                "{path} belongs to the server; uploads never replace it. Edit it instead."
            ),
        ));
    }
    Ok(())
}

/// `POST fs/create`: a folder, or a file with the body as its contents. Never replaces a
/// server-owned file.
pub async fn create(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<CreateQuery>,
    body: Body,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let path = inner_path(&query.path)?;
    match query.kind {
        CreateKind::Directory => {
            let (folder, _busy) = changing(&state, &id)?;
            blocking(move || Ok(folder.create_dir_all(&path)?)).await?;
        }
        CreateKind::File => {
            let staged = stage_body(&state, &id, body).await?;
            place_changing(&state, &id, path, staged, false).await?;
        }
    }
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `PUT fs/update`: replace a file's contents with the body. An explicit edit, so it may change a
/// server-owned file.
pub async fn update(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<PathQuery>,
    body: Body,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let path = inner_path(&query.path)?;
    let staged = stage_body(&state, &id, body).await?;
    place_changing(&state, &id, path, staged, true).await?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST fs/move`. Never onto something that exists.
pub async fn rename(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<MoveRequest>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let from = inner_path(&request.source)?;
    let to = inner_path(&request.destination)?;
    let (folder, _busy) = changing(&state, &id)?;
    blocking(move || {
        if !folder.exists(&from) {
            return Err(not_found(&from));
        }
        if folder.exists(&to) {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                format!("{to} already exists."),
            ));
        }
        Ok(folder.rename(&from, &to)?)
    })
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE fs/delete`. A folder with something in it needs `recursive`.
pub async fn delete(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<DeleteQuery>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let path = inner_path(&query.path)?;
    let (folder, _busy) = changing(&state, &id)?;
    blocking(move || {
        let meta = folder.metadata(&path).map_err(|_| not_found(&path))?;
        if meta.is_dir() && !query.recursive {
            folder.remove_empty_dir(&path).map_err(|_| {
                ApiError::new(
                    ErrorCode::Conflict,
                    format!("{path} is not empty."),
                )
            })?;
        } else {
            folder.remove(&path)?;
        }
        Ok(())
    })
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST v1/fs/unarchive`: unpack a zip in the server folder into `trg`. `dry` only lists the
/// files that exist already; otherwise they are replaced when `override` says so, except
/// server-owned ones, which are never replaced. A pack's name is reported so the App can offer to
/// install it as one.
pub async fn unarchive(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<UnarchiveQuery>,
) -> Result<Json<ExtractResult>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let source = inner_path(&query.src)?;
    let target = api_path(&query.trg)?;
    let (folder, _busy) = changing(&state, &id)?;
    let result = blocking(move || {
        let file = folder
            .open_file(&source)
            .ok()
            .flatten()
            .ok_or_else(|| not_found(&source))?
            .into_std();
        let mut zip = zip::ZipArchive::new(file).map_err(|error| {
            ApiError::from(crate::content::Error::from(ArchiveError::from(
                error,
            )))
        })?;
        let modpack_name =
            pack::read_index(&mut zip).ok().map(|index| index.name);
        let level = folder.level_name();
        let destination = |entry: &str| match &target {
            Some(target) => format!("{target}/{entry}"),
            None => entry.to_owned(),
        };
        let entries =
            archive::entries(&mut zip).map_err(crate::content::Error::from)?;
        let conflicting_files: Vec<String> = entries
            .iter()
            .filter(|(_, is_dir)| !is_dir)
            .map(|(entry, _)| destination(entry))
            .filter(|path| folder.exists(path))
            .map(|path| format!("/{path}"))
            .collect();
        if !query.dry {
            archive::extract(&folder, &mut zip, |entry| {
                let path = destination(entry);
                let exists = folder.exists(&path);
                let owned = folder::is_server_owned(&path, &level);
                (!exists || (query.overwrite && !owned)).then_some(path)
            })
            .map_err(crate::content::Error::from)?;
        }
        Ok(ExtractResult {
            modpack_name,
            conflicting_files,
        })
    })
    .await?;
    if !query.dry {
        activity::log(&state.db, &caller, Some(&id), Action::ContentChanged)
            .await;
    }
    Ok(Json(result))
}

/// `POST v1/fs/ops/{action}`: unpacking finishes within its request, so there is never an
/// operation left to dismiss or cancel.
pub async fn operation(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, _action)): Path<(String, OperationAction)>,
    Query(_query): Query<OperationQuery>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST v1/worlds/{wid}/files/stat`.
pub async fn stat(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<FileStatRequest>,
) -> Result<Json<FileStatResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let path = api_path(&request.path)?;
    let folder = folder(&state, &id)?;
    let response = blocking(move || {
        let meta = match &path {
            Some(path) => folder.metadata(path),
            None => folder.open_dir(None).and_then(|dir| dir.dir_metadata()),
        }
        .map_err(|_| not_found(path.as_deref().unwrap_or("folder")))?;
        let time = |time: std::io::Result<cap_std::time::SystemTime>| {
            DateTime::<Utc>::from_timestamp(unix(time), 0).unwrap_or_default()
        };
        let name = path
            .as_deref()
            .and_then(|path| path.rsplit('/').next())
            .unwrap_or_default()
            .to_owned();
        Ok(FileStatResponse {
            name,
            full_path: format!("/{}", path.unwrap_or_default()),
            size_bytes: meta.len(),
            kind: if meta.file_type().is_symlink() {
                DescendantType::Symlink
            } else if meta.is_dir() {
                DescendantType::Directory
            } else if meta.is_file() {
                DescendantType::Regular
            } else {
                DescendantType::Other
            },
            mtime: time(meta.modified()),
            ctime: time(meta.created()),
        })
    })
    .await?;
    Ok(Json(response))
}

/// Add `dir` to `zip` under `prefix`, skipping links.
fn zip_tree(
    zip: &mut zip::ZipWriter<std::fs::File>,
    dir: &cap_std::fs::Dir,
    prefix: &str,
) -> Result<(), ApiError> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    for entry in dir.entries()?.filter_map(Result::ok) {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let kind = entry.file_type()?;
        let path = format!("{prefix}{name}");
        if kind.is_dir() {
            zip.add_directory(format!("{path}/"), options)
                .map_err(zip_error)?;
            zip_tree(
                zip,
                &folder::open_child(dir, &name, &path)?,
                &format!("{path}/"),
            )?;
        } else if kind.is_file() {
            zip.start_file(&path, options).map_err(zip_error)?;
            std::io::copy(&mut folder::open_plain(dir, &name, &path)?, zip)?;
        }
    }
    Ok(())
}

fn zip_error(error: zip::result::ZipError) -> ApiError {
    ApiError::new(ErrorCode::Internal, error.to_string())
}

/// `POST v1/worlds/{wid}/files/zip`: zip a folder into `<folder>.zip`, or paths inside `parent`
/// into `target`. Answers with one `application/json-seq` record once the zip is written.
pub async fn zip(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<ZipRequest>,
) -> Result<Response, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let (parent, include, target) = match request {
        ZipRequest::Directory { path } => {
            let path = inner_path(&path)?;
            let (parent, name) = path
                .rsplit_once('/')
                .map_or((None, path.clone()), |(parent, name)| {
                    (Some(parent.to_owned()), name.to_owned())
                });
            (parent, vec![name], format!("{path}.zip"))
        }
        ZipRequest::ManyPaths {
            parent,
            include,
            target,
        } => {
            let include = include
                .iter()
                .map(|name| folder::clean(name.trim_matches('/')))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    ApiError::new(ErrorCode::InvalidRequest, error.to_string())
                })?;
            (api_path(&parent)?, include, inner_path(&target)?)
        }
    };
    let (folder, _busy) = changing(&state, &id)?;
    refuse_owned(&folder, &target)?;
    let staged = state
        .servers
        .paths()
        .staging(&id)
        .join(format!("zip-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(staged.parent().expect("inside staging")).await?;
    let result = blocking({
        let staged = staged.clone();
        let folder = Arc::clone(&folder);
        move || {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&staged)?);
            for name in &include {
                let path = match &parent {
                    Some(parent) => format!("{parent}/{name}"),
                    None => name.clone(),
                };
                let meta =
                    folder.metadata(&path).map_err(|_| not_found(name))?;
                if meta.is_dir() {
                    zip.add_directory(
                        format!("{name}/"),
                        zip::write::SimpleFileOptions::default(),
                    )
                    .map_err(zip_error)?;
                    zip_tree(
                        &mut zip,
                        &folder.open_dir(Some(&path))?,
                        &format!("{name}/"),
                    )?;
                } else if meta.is_file() {
                    zip.start_file(
                        name,
                        zip::write::SimpleFileOptions::default()
                            .large_file(true),
                    )
                    .map_err(zip_error)?;
                    let mut file = folder
                        .open_file(&path)?
                        .ok_or_else(|| not_found(name))?;
                    std::io::copy(&mut file, &mut zip)?;
                }
            }
            zip.finish().map_err(zip_error)?;
            folder.write(&target, &mut std::fs::File::open(&staged)?)?;
            Ok(())
        }
    })
    .await;
    let _ = tokio::fs::remove_file(&staged).await;
    let record = match result {
        Ok(()) => ZipProgress {
            progress: 1.0,
            done: Some(true),
            error: None,
        },
        Err(error) => ZipProgress {
            progress: 0.0,
            done: Some(true),
            error: Some(error.description),
        },
    };
    let body = format!(
        "\u{1e}{}\n",
        serde_json::to_string(&record).expect("serializes")
    );
    Ok(
        ([(header::CONTENT_TYPE, "application/json-seq")], body)
            .into_response(),
    )
}

/// `POST v1/worlds/{wid}/{scope}/upload-session`: open a session for the caller, ending any open
/// one they had for this world and scope.
pub async fn open_session(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, scope)): Path<(String, String, UploadScope)>,
) -> Result<Json<UploadSessionResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let upload_id = uuid::Uuid::new_v4().to_string();
    let now = now();
    let session = Session {
        server_id: id.clone(),
        scope,
        user_id: caller.user_id().map(str::to_owned),
        status: UploadSessionStatus::Active,
        created_at: now,
        updated_at: now,
        last_upload_at: None,
        dir: state
            .servers
            .paths()
            .staging(&id)
            .join(format!("session-{upload_id}")),
        files: Vec::new(),
        bytes: 0,
    };
    let response = session.response(&upload_id);
    let ended: Vec<PathBuf> = {
        let mut sessions = state.uploads.lock();
        let ended = sessions
            .values_mut()
            .filter(|other| {
                other.open() && other.belongs_to(&caller, &id, scope)
            })
            .map(|other| {
                other.end(UploadSessionStatus::Cancelled);
                other.dir.clone()
            })
            .collect();
        sessions.insert(upload_id, session);
        ended
    };
    for dir in ended {
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
    Ok(Json(response))
}

/// `GET v1/worlds/{wid}/{scope}/upload-session`: the caller's open session, if any.
pub async fn get_session(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, scope)): Path<(String, String, UploadScope)>,
) -> Result<Json<GetUploadSessionResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let session = state
        .uploads
        .lock()
        .iter()
        .find(|(_, session)| {
            session.open() && session.belongs_to(&caller, &id, scope)
        })
        .map(|(upload_id, session)| session.response(upload_id));
    Ok(Json(GetUploadSessionResponse { session }))
}

/// `POST v1/worlds/{wid}/{scope}/upload-session/{upload}/files`: stage the `file` parts. Each
/// part's file name is where it goes: a path in the server folder for `files`, a file name for
/// `content`. The files count only if the session is still open, and within its cap, once they
/// have all arrived.
pub async fn upload_files(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, scope, upload_id)): Path<(
        String,
        String,
        UploadScope,
        String,
    )>,
    mut form: Multipart,
) -> Result<Json<UploadSessionResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let (dir, budget) =
        state
            .uploads
            .change(&caller, &id, scope, &upload_id, |session| {
                Ok((
                    session.dir.clone(),
                    MAX_UPLOAD.saturating_sub(session.bytes),
                ))
            })?;
    let mut added = Vec::new();
    let mut remaining = budget;
    let result = async {
        while let Some(mut field) =
            form.next_field().await.map_err(|error| {
                ApiError::new(ErrorCode::InvalidRequest, error.body_text())
            })?
        {
            let Some(name) = field.file_name().map(str::to_owned) else {
                continue;
            };
            // Checked now, so a bad name fails before it is stored.
            match scope {
                UploadScope::Files => {
                    inner_path(&name)?;
                }
                UploadScope::Content => {
                    folder::clean(&name).map_err(|error| {
                        ApiError::new(
                            ErrorCode::InvalidRequest,
                            error.to_string(),
                        )
                    })?;
                }
            }
            let staged = dir.join(uuid::Uuid::new_v4().to_string());
            stage_field(&mut field, &staged, remaining).await?;
            let size = tokio::fs::metadata(&staged).await?.len();
            remaining = remaining.saturating_sub(size);
            added.push((name, staged, size));
        }
        Ok::<_, ApiError>(())
    }
    .await;
    let committed =
        result.and_then(|()| {
            state
                .uploads
                .change(&caller, &id, scope, &upload_id, |session| {
                    let size: u64 = added.iter().map(|(_, _, size)| size).sum();
                    if session.bytes + size > MAX_UPLOAD {
                        return Err(ApiError::new(
                            ErrorCode::InvalidRequest,
                            "Uploads are at most 4 GB.",
                        ));
                    }
                    let now = now();
                    session.files.extend(added.iter().map(
                        |(name, staged, _)| (name.clone(), staged.clone()),
                    ));
                    session.bytes += size;
                    session.status = UploadSessionStatus::Uploading;
                    session.updated_at = now;
                    session.last_upload_at = Some(now);
                    Ok(session.response(&upload_id))
                })
        });
    if committed.is_err() {
        for (_, staged, _) in &added {
            let _ = tokio::fs::remove_file(staged).await;
        }
    }
    Ok(Json(committed?))
}

/// `POST .../upload-session/{upload}/finalize`: put every staged file in place. Files go where
/// their names say, never over a server-owned file; content goes through the content model.
pub async fn finalize_session(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, scope, upload_id)): Path<(
        String,
        String,
        UploadScope,
        String,
    )>,
) -> Result<Json<UploadSessionResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    // Taken in one step, so a second finalize finds the session over.
    let (files, dir) =
        state
            .uploads
            .change(&caller, &id, scope, &upload_id, |session| {
                session.status = UploadSessionStatus::Finalizing;
                session.updated_at = now();
                Ok((session.files.clone(), session.dir.clone()))
            })?;
    let result = async {
        match scope {
            UploadScope::Files => {
                let (folder, _busy) = changing(&state, &id)?;
                for (name, _) in &files {
                    refuse_owned(&folder, &inner_path(name)?)?;
                }
                for (name, staged) in &files {
                    place(
                        Arc::clone(&folder),
                        inner_path(name)?,
                        staged.clone(),
                    )
                    .await?;
                }
            }
            UploadScope::Content => {
                let _busy = state.content.busy(&id)?;
                for (name, staged) in &files {
                    addons::upload(&state.content, &id, name, staged).await?;
                }
            }
        }
        Ok::<_, ApiError>(())
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    let status = if result.is_ok() {
        UploadSessionStatus::Finalized
    } else {
        UploadSessionStatus::Cancelled
    };
    let response = state.uploads.lock().get_mut(&upload_id).map(|session| {
        let bytes = session.bytes;
        let count = session.files.len();
        session.end(status);
        let mut response = session.response(&upload_id);
        response.entry_count = count as u32;
        response.uploaded_byte_count = bytes;
        response
    });
    result?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    response.map(Json).ok_or_else(|| {
        ApiError::new(ErrorCode::NotFound, "There is no such upload.")
    })
}

/// `DELETE .../upload-session/{upload}`: cancel it and remove what it staged. A session being
/// finalized can no longer be cancelled.
pub async fn cancel_session(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, scope, upload_id)): Path<(
        String,
        String,
        UploadScope,
        String,
    )>,
) -> Result<Json<UploadSessionResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let (response, dir) =
        state
            .uploads
            .change(&caller, &id, scope, &upload_id, |session| {
                session.end(UploadSessionStatus::Cancelled);
                Ok((session.response(&upload_id), session.dir.clone()))
            })?;
    let _ = tokio::fs::remove_dir_all(dir).await;
    Ok(Json(response))
}
