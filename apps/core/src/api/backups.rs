//! The queued backup engine, under `/v1/servers/{id}/worlds/{wid}/backups-queue`: list, create,
//! `delete-many`, `{bid}` delete, `{bid}/restore`, and `history/{kind}/{operation}/{action}`; and
//! `GET /servers/{id}/modrinth/v0/backups/{bid}/download`. Mirrors `Archon.BackupsQueue.v1` in
//! `packages/api-client/src/modules/archon/types.ts` (`archon/backups-queue/v1.ts`) and
//! `getBackupDownloadUrl` in `utils/node-url.ts`. Also the world download,
//! `GET /servers/{id}/v1/worlds/{wid}/files/download-full-zip` and its `authorize`
//! (`kyros/files/v1.ts`), `GET /v1/servers/{id}/worlds/{wid}/select-download`, and Amberite's
//! `GET|PUT /servers/{id}/backups/schedule` and `.../backups/{bid}/download/authorize`.
//!
//! Viewers see the list and the schedule. Editors create backups, download them and the world,
//! and set the schedule. Restoring and deleting are the owner's, with a fresh ticket, and so is
//! keeping fewer scheduled backups, which deletes the oldest at the next one.

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{
    AppState,
    activity::Action,
    content::world,
    error::{ApiError, ErrorCode},
    servers::WorldDownloadMethod,
};
use crate::{
    access::{Caller, Dangerous, Level, activity},
    backups::{
        self, Target,
        records::{
            self, Kind, OpKind, OpState, Operation, State as BackupRowState,
        },
    },
    servers::{ServerError, records as servers_records},
};

/// Also the `{kind}` of the history routes. Mirrors
/// Archon.BackupsQueue.v1.BackupQueueOperationType.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupQueueOperationType {
    Create,
    Restore,
}

/// Mirrors Archon.BackupsQueue.v1.BackupQueueState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupQueueState {
    Pending,
    Ongoing,
    Completed,
    Cancelled,
    Failed,
    #[allow(dead_code)] // Core waits out a slow save and copies anyway.
    TimedOut,
}

/// Mirrors Archon.BackupsQueue.v1.BackupStatus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupStatus {
    Pending,
    InProgress,
    #[allow(dead_code)] // Core waits out a slow save and copies anyway.
    TimedOut,
    Error,
    Done,
}

/// Body of POST `backups-queue` (create) and POST `backups-queue/{bid}/restore`. Mirrors
/// Archon.BackupsQueue.v1.BackupRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupRequest {
    pub name: String,
}

/// Response of POST `backups-queue`. Mirrors Archon.BackupsQueue.v1.PostBackupQueueResponse.
#[derive(Debug, Clone, Serialize)]
pub struct PostBackupQueueResponse {
    pub id: Uuid,
}

/// Who started an operation. Mirrors Archon.BackupsQueue.v1.UserInfo.
#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

/// Body of POST `backups-queue/delete-many`. Mirrors
/// Archon.BackupsQueue.v1.DeleteManyBackupRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteManyBackupRequest {
    pub backup_ids: Vec<Uuid>,
}

/// The `{action}` of POST `backups-queue/history/{kind}/{operation}/{action}`: `ack` dismisses a
/// finished operation, `cancel` stops a queued one. Mirrors the `ackCreate`, `cancelCreate`,
/// `ackRestore`, and `cancelRestore` paths of `ArchonBackupsQueueV1Module`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryAction {
    Ack,
    Cancel,
}

/// A queued or running operation. Mirrors Archon.BackupsQueue.v1.ActiveOperation.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveOperation {
    pub backup_id: Uuid,
    pub operation_type: BackupQueueOperationType,
    pub operation_id: Option<i64>,
    pub has_parent: bool,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub synthetic_legacy: bool,
    pub user_info: Option<UserInfo>,
}

/// One past or current operation on a backup. Mirrors
/// Archon.BackupsQueue.v1.BackupQueueOperation.
#[derive(Debug, Clone, Serialize)]
pub struct BackupQueueOperation {
    pub operation_type: BackupQueueOperationType,
    pub operation_id: Option<i64>,
    pub state: BackupQueueState,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub has_parent: bool,
    pub error: Option<String>,
    pub should_prompt: bool,
    pub synthetic_legacy: bool,
    pub user_info: Option<UserInfo>,
}

/// Mirrors Archon.BackupsQueue.v1.BackupQueueBackup.
#[derive(Debug, Clone, Serialize)]
pub struct BackupQueueBackup {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub status: BackupStatus,
    pub locked: bool,
    pub automated: bool,
    pub history: Vec<BackupQueueOperation>,
}

/// Response of GET `backups-queue`. Mirrors Archon.BackupsQueue.v1.BackupsQueueResponse.
#[derive(Debug, Clone, Serialize)]
pub struct BackupsQueueResponse {
    pub active_operations: Vec<ActiveOperation>,
    pub backups: Vec<BackupQueueBackup>,
}

/// Query of GET `/servers/{id}/modrinth/v0/backups/{bid}/download`. A browser follows the link, so
/// the token travels in the query. Mirrors the `auth` param of `getBackupDownloadUrl`.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupDownloadQuery {
    #[serde(default)]
    pub auth: Option<String>,
}

/// Response of the download `authorize` routes: a link token for one download in the next five
/// minutes. Mirrors Kyros.Files.v1.FullWorldDownloadAuthorization.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadToken {
    pub token: String,
}

/// Query of `GET /servers/{id}/v1/worlds/{wid}/files/download-full-zip`. Mirrors the `token`
/// param of `KyrosFilesV1Module.getFullWorldDownloadUrl`.
#[derive(Debug, Clone, Deserialize)]
pub struct WorldDownloadQuery {
    #[serde(default)]
    pub token: Option<String>,
}

/// Body of `PUT /servers/{id}/backups/schedule`, and the response of its `GET`: hours between
/// scheduled backups (0 turns them off) and how many to keep. Without one set: 24 and 7.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
pub struct BackupSchedule {
    pub interval_hours: u32,
    pub keep: u32,
}

impl From<backups::Error> for ApiError {
    fn from(error: backups::Error) -> Self {
        use backups::Error;
        match error {
            Error::Server(error) => error.into(),
            error => {
                let code = match &error {
                    Error::NotFound(_) => ErrorCode::NotFound,
                    Error::Conflict(_) | Error::Space { .. } => {
                        ErrorCode::Conflict
                    }
                    _ => ErrorCode::Internal,
                };
                Self::new(code, format!("{error:#}"))
            }
        }
    }
}

fn uuid(id: &str) -> Uuid {
    Uuid::parse_str(id).unwrap_or_default()
}

fn time(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds, 0).unwrap_or_default()
}

fn operation_type(kind: OpKind) -> BackupQueueOperationType {
    match kind {
        OpKind::Create => BackupQueueOperationType::Create,
        OpKind::Restore => BackupQueueOperationType::Restore,
    }
}

/// Who started each operation, by user id, as the last ticket for them named them.
async fn users(
    state: &AppState,
    operations: &[Operation],
) -> Result<HashMap<String, UserInfo>, ApiError> {
    let mut users = HashMap::new();
    for user_id in operations.iter().filter_map(|op| op.user_id.as_ref()) {
        if users.contains_key(user_id) {
            continue;
        }
        let row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT name, avatar_url FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&state.db)
                .await?;
        let (username, avatar_url) =
            row.unwrap_or_else(|| (user_id.clone(), None));
        users.insert(
            user_id.clone(),
            UserInfo {
                id: user_id.clone(),
                username,
                avatar_url,
            },
        );
    }
    Ok(users)
}

/// `GET .../backups-queue`: every backup with its history, and what is queued or running.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<BackupsQueueResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    world(&id, &wid)?;
    let operations = records::operations(&state.db, &id).await?;
    let users = users(&state, &operations).await?;
    let user = |op: &Operation| {
        op.user_id
            .as_ref()
            .and_then(|user| users.get(user))
            .cloned()
    };
    let active_operations = operations
        .iter()
        .filter(|op| matches!(op.state, OpState::Queued | OpState::Running))
        .map(|op| ActiveOperation {
            backup_id: uuid(&op.backup_id),
            operation_type: operation_type(op.kind),
            operation_id: Some(op.id),
            has_parent: false,
            scheduled_for: time(op.queued_at),
            started_at: op.started_at.map(time),
            synthetic_legacy: false,
            user_info: user(op),
        })
        .collect();
    let backups = records::backups(&state.db, &id)
        .await?
        .into_iter()
        .map(|backup| BackupQueueBackup {
            id: uuid(&backup.id),
            name: backup.name,
            created_at: time(backup.created_at),
            status: match backup.state {
                BackupRowState::Queued => BackupStatus::Pending,
                BackupRowState::Running => BackupStatus::InProgress,
                BackupRowState::Done => BackupStatus::Done,
                BackupRowState::Failed => BackupStatus::Error,
            },
            locked: false,
            automated: backup.kind != Kind::Manual,
            history: operations
                .iter()
                .filter(|op| op.backup_id == backup.id)
                .map(|op| {
                    let finished = matches!(
                        op.state,
                        OpState::Done | OpState::Failed | OpState::Cancelled
                    );
                    BackupQueueOperation {
                        operation_type: operation_type(op.kind),
                        operation_id: Some(op.id),
                        state: match op.state {
                            OpState::Queued => BackupQueueState::Pending,
                            OpState::Running => BackupQueueState::Ongoing,
                            OpState::Done => BackupQueueState::Completed,
                            OpState::Failed => BackupQueueState::Failed,
                            OpState::Cancelled => BackupQueueState::Cancelled,
                        },
                        scheduled_for: time(op.queued_at),
                        started_at: op.started_at.map(time),
                        completed_at: op.ended_at.map(time),
                        has_parent: false,
                        error: op.error.clone(),
                        should_prompt: finished && !op.acked,
                        synthetic_legacy: false,
                        user_info: user(op),
                    }
                })
                .collect(),
        })
        .collect();
    Ok(Json(BackupsQueueResponse {
        active_operations,
        backups,
    }))
}

/// `POST .../backups-queue`: queue a backup of the server as it is, hot when it runs.
pub async fn create(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<BackupRequest>,
) -> Result<Json<PostBackupQueueResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "A backup name has 1 to 64 characters.",
        ));
    }
    let (backup, _) = state
        .backups
        .create(&id, name, Kind::Manual, caller.user_id())
        .await?;
    activity::log(
        &state.db,
        &caller,
        Some(&id),
        Action::BackupCreated {
            backup_id: uuid(&backup),
        },
    )
    .await;
    Ok(Json(PostBackupQueueResponse { id: uuid(&backup) }))
}

/// `DELETE .../backups-queue/{bid}`: owner only, with a fresh ticket, since backups are what
/// undoes damage.
pub async fn delete(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
    Path((id, wid, backup)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    world(&id, &wid)?;
    state.backups.delete(&id, &backup).await?;
    activity::log(
        &state.db,
        &caller,
        Some(&id),
        Action::BackupDeleted {
            backup_id: uuid(&backup),
        },
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST .../backups-queue/delete-many`: as `delete`, for several.
pub async fn delete_many(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<DeleteManyBackupRequest>,
) -> Result<StatusCode, ApiError> {
    world(&id, &wid)?;
    for backup in &request.backup_ids {
        state.backups.delete(&id, &backup.to_string()).await?;
        activity::log(
            &state.db,
            &caller,
            Some(&id),
            Action::BackupDeleted { backup_id: *backup },
        )
        .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST .../backups-queue/{bid}/restore`: queue a restore over the world of the stopped
/// server. A dangerous action: owner only, with a fresh ticket.
pub async fn restore(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
    Path((id, wid, backup)): Path<(String, String, String)>,
    Json(_request): Json<BackupRequest>,
) -> Result<StatusCode, ApiError> {
    world(&id, &wid)?;
    state
        .backups
        .restore(&id, &backup, caller.user_id())
        .await?;
    activity::log(
        &state.db,
        &caller,
        Some(&id),
        Action::BackupRestored {
            backup_id: uuid(&backup),
        },
    )
    .await;
    Ok(StatusCode::ACCEPTED)
}

/// `POST .../backups-queue/{bid}/retry`: queue a failed backup again.
pub async fn retry(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, backup)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    state.backups.retry(&id, &backup, caller.user_id()).await?;
    Ok(StatusCode::ACCEPTED)
}

/// `POST .../backups-queue/history/{kind}/{operation}/{action}`: `ack` stops showing a finished
/// operation, `cancel` drops a queued one (a cancelled create takes its backup with it).
pub async fn history(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, kind, operation, action)): Path<(
        String,
        String,
        BackupQueueOperationType,
        i64,
        HistoryAction,
    )>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let found = records::operation(&state.db, operation)
        .await?
        .filter(|op| op.server_id == id && operation_type(op.kind) == kind);
    if found.is_none() {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "There is no such operation.",
        ));
    }
    let done = match action {
        HistoryAction::Ack => records::ack(&state.db, &id, operation).await?,
        HistoryAction::Cancel => {
            records::cancel(&state.db, &id, operation).await?
        }
    };
    if !done {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            match action {
                HistoryAction::Ack => "The operation has not finished.",
                HistoryAction::Cancel => {
                    "Only a queued operation can be cancelled."
                }
            },
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /servers/{id}/modrinth/v0/backups/{bid}/download/authorize`: a link token for the
/// backup's zip, for a browser that cannot send the session.
pub async fn authorize_backup(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, backup)): Path<(String, String)>,
) -> Result<Json<DownloadToken>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    records::backup(&state.db, &id, &backup)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::NotFound,
                format!("There is no backup {backup}."),
            )
        })?;
    let token = state.backups.grant(caller, &id, Target::Backup(backup));
    Ok(Json(DownloadToken { token }))
}

/// `GET /servers/{id}/v1/worlds/{wid}/files/download-full-zip/authorize`: a link token for the
/// world as it is now.
pub async fn authorize_world(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<DownloadToken>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let token = state.backups.grant(caller, &id, Target::World);
    Ok(Json(DownloadToken { token }))
}

/// Who is downloading: a link token for `target` on `server`, or a session in the header. Either
/// way they must still be an editor of the server.
async fn downloader(
    state: &AppState,
    caller: Result<Caller, ApiError>,
    token: Option<&str>,
    server: &str,
    target: &Target,
) -> Result<Caller, ApiError> {
    let caller = match token {
        Some(token) => state
            .backups
            .take(token)
            .filter(|grant| {
                grant.server_id == server && grant.target == *target
            })
            .map(|grant| grant.caller)
            .ok_or_else(|| {
                ApiError::new(
                    ErrorCode::Unauthorized,
                    "This download link has expired or was used.",
                )
            })?,
        None => caller?,
    };
    caller.require(&state.db, server, Level::Editor).await?;
    Ok(caller)
}

/// A zip streamed to the client, with a name to save it as.
fn zip_response(
    file: std::fs::File,
    name: &str,
    temporary: Option<std::path::PathBuf>,
) -> Response {
    let length = file.metadata().map(|meta| meta.len()).unwrap_or_default();
    let stream = Temporary {
        inner: Some(tokio_util::io::ReaderStream::new(
            tokio::fs::File::from_std(file),
        )),
        path: temporary,
    };
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    (
        [
            (header::CONTENT_TYPE, "application/zip".to_owned()),
            (header::CONTENT_LENGTH, length.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}.zip\""),
            ),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

/// A file stream that removes its file, when it has one to remove, once the response is done
/// with it, sent or not.
struct Temporary {
    inner: Option<tokio_util::io::ReaderStream<tokio::fs::File>>,
    path: Option<std::path::PathBuf>,
}

impl futures::Stream for Temporary {
    type Item = std::io::Result<bytes::Bytes>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.inner.as_mut() {
            Some(inner) => std::pin::Pin::new(inner).poll_next(cx),
            None => std::task::Poll::Ready(None),
        }
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        // Closed first: Windows cannot remove an open file.
        self.inner.take();
        if let Some(path) = &self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// `GET /servers/{id}/modrinth/v0/backups/{bid}/download`: a backup's zip, streamed. The link
/// token goes in `auth`; a session in the header works too.
pub async fn download_backup(
    caller: Result<Caller, ApiError>,
    State(state): State<Arc<AppState>>,
    Path((id, backup)): Path<(String, String)>,
    Query(query): Query<BackupDownloadQuery>,
) -> Result<Response, ApiError> {
    let target = Target::Backup(backup.clone());
    let caller =
        downloader(&state, caller, query.auth.as_deref(), &id, &target).await?;
    let found = records::backup(&state.db, &id, &backup)
        .await?
        .filter(|found| found.state == BackupRowState::Done)
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::NotFound,
                format!("There is no finished backup {backup}."),
            )
        })?;
    let file = std::fs::File::open(state.backups.file(&id, &backup))?;
    activity::log(
        &state.db,
        &caller,
        Some(&id),
        Action::BackupDownloaded {
            backup_id: uuid(&backup),
        },
    )
    .await;
    Ok(zip_response(file, &found.name, None))
}

/// `GET /servers/{id}/v1/worlds/{wid}/files/download-full-zip`: the world and settings as they
/// are now, copied as a backup is (hot when the server runs) and streamed, then removed.
pub async fn download_world(
    caller: Result<Caller, ApiError>,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Query(query): Query<WorldDownloadQuery>,
) -> Result<Response, ApiError> {
    world(&id, &wid)?;
    let caller =
        downloader(&state, caller, query.token.as_deref(), &id, &Target::World)
            .await?;
    let server = servers_records::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let path = state
        .servers
        .paths()
        .backups()
        .join(&id)
        .join(format!("download-{}.zip", Uuid::new_v4()));
    state.backups.snapshot(&id, &path, |_| {}).await?;
    let file = std::fs::File::open(&path)?;
    activity::log(&state.db, &caller, Some(&id), Action::WorldDownloaded).await;
    Ok(zip_response(file, &server.name, Some(path)))
}

/// `GET /v1/servers/{id}/worlds/{wid}/select-download`: the world downloads straight from Core.
pub async fn select_download(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<WorldDownloadMethod>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    Ok(Json(WorldDownloadMethod::DirectNodeDownload))
}

/// `GET /servers/{id}/backups/schedule`.
pub async fn get_schedule(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<BackupSchedule>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    let schedule = records::schedule(&state.db, &id).await?;
    Ok(Json(BackupSchedule {
        interval_hours: schedule.interval_hours,
        keep: schedule.keep,
    }))
}

/// `PUT /servers/{id}/backups/schedule`: at most every hour and at least monthly, keeping 1 to
/// 100; an interval of 0 turns scheduled backups off. Editors set it; keeping fewer than now is
/// the owner's, with a fresh ticket, since the next scheduled backup then deletes the oldest,
/// as deleting them by hand would need.
pub async fn set_schedule(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<BackupSchedule>,
) -> Result<Json<BackupSchedule>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    if body.interval_hours > 24 * 31 || !(1..=100).contains(&body.keep) {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "Back up at most every 744 hours (0 is off), keeping 1 to 100.",
        ));
    }
    servers_records::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let current = records::schedule(&state.db, &id).await?;
    let caller = if body.keep < current.keep {
        Dangerous::check(&state, caller, &headers).await?.0
    } else {
        caller
    };
    records::set_schedule(
        &state.db,
        &id,
        records::Schedule {
            interval_hours: body.interval_hours,
            keep: body.keep,
        },
    )
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(body))
}
