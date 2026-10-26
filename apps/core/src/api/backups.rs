//! The queued backup engine, under `/v1/servers/{id}/worlds/{wid}/backups-queue`: list, create,
//! `delete-many`, `{bid}` delete, `{bid}/restore`, `{bid}/retry`, and
//! `history/{kind}/{operation}/{action}`; `with-options` (GET and PATCH), which adds the slot
//! options and the next scheduled backup; and `PATCH /v1/servers/{id}/worlds/{wid}/backups/{bid}`,
//! which renames. Mirrors `Archon.BackupsQueue.v1` and `Archon.Backups.v1.PatchBackup` in
//! `packages/api-client/src/modules/archon/types.ts` (`archon/backups-queue/v1.ts`,
//! `archon/backups/v1.ts`). Also the downloads a browser follows with a link token:
//! `GET /servers/{id}/v1/worlds/{wid}/backups/{bid}/download` and
//! `.../files/download-full-zip`, each with its `authorize` (`kyros/backups/v1.ts`,
//! `kyros/files/v1.ts`), and `GET /v1/servers/{id}/worlds/{wid}/select-download`.
//!
//! Core has [`SLOTS`] backup slots per server, split between scheduled and manual backups as the
//! App's backup settings show them; the schedule's `keep` is the scheduled share.
//!
//! Viewers see the list and the options. Editors create, rename, and download backups and the
//! world, and set the options. Restoring and deleting are the owner's, with a fresh ticket, and so
//! is giving scheduled backups fewer slots than are in use, which deletes the oldest.

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
        self, SLOTS, Target,
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

/// Body of PATCH `backups/{bid}`. Mirrors Archon.Backups.v1.PatchBackup.
#[derive(Debug, Clone, Deserialize)]
pub struct PatchBackup {
    #[serde(default)]
    pub name: Option<String>,
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

/// Mirrors Archon.BackupsQueue.v1.BackupQueueBackup. `rotates_at` is set on the backups that
/// hold a scheduled slot: when the scheduled backups after it will have pushed it out.
#[derive(Debug, Clone, Serialize)]
pub struct BackupQueueBackup {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub status: BackupStatus,
    pub locked: bool,
    pub automated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotates_at: Option<DateTime<Utc>>,
    pub history: Vec<BackupQueueOperation>,
}

/// Response of GET `backups-queue`. Mirrors Archon.BackupsQueue.v1.BackupsQueueResponse.
#[derive(Debug, Clone, Serialize)]
pub struct BackupsQueueResponse {
    pub active_operations: Vec<ActiveOperation>,
    pub backups: Vec<BackupQueueBackup>,
}

/// How a server's [`SLOTS`] are split and how often it is backed up. Mirrors
/// Archon.BackupsQueue.v1.WorldBackupOptions.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct WorldBackupOptions {
    pub split_automatic: u32,
    pub split_manual: u32,
    pub limit: u32,
    pub interval: u32,
}

/// Response of GET `backups-queue/with-options`. Mirrors
/// Archon.BackupsQueue.v1.BackupsQueueWithOptionsResponse.
#[derive(Debug, Clone, Serialize)]
pub struct BackupsQueueWithOptionsResponse {
    #[serde(flatten)]
    pub queue: BackupsQueueResponse,
    pub options: WorldBackupOptions,
    pub next_auto_backup_at: Option<DateTime<Utc>>,
}

/// Body of PATCH `backups-queue/with-options`. Mirrors
/// Archon.BackupsQueue.v1.PatchWorldBackupOptions.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PatchWorldBackupOptions {
    #[serde(default)]
    pub split_automatic: Option<u32>,
    #[serde(default)]
    pub split_manual: Option<u32>,
    #[serde(default)]
    pub interval: Option<u32>,
}

/// Query of PATCH `backups-queue/with-options`: `ar=true` acknowledges that the scheduled
/// backups past the new slot count are deleted. Mirrors the `ar` param of
/// `ArchonBackupsQueueV1Module.patchOptions`.
#[derive(Debug, Clone, Deserialize)]
pub struct PatchOptionsQuery {
    #[serde(default)]
    pub ar: Option<String>,
}

/// Response of PATCH `backups-queue/with-options`. Not `applied` when the change would delete
/// the scheduled backups in `backup_ids` and was not acknowledged. Mirrors
/// Archon.BackupsQueue.v1.PatchWorldBackupOptionsResponse.
#[derive(Debug, Clone, Serialize)]
pub struct PatchWorldBackupOptionsResponse {
    pub applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_ids: Option<Vec<Uuid>>,
}

/// Response of the download `authorize` routes: a link token for one download in the next five
/// minutes. Mirrors Kyros.Backups.v1.BackupDownloadAuthorization and the
/// Kyros.Files.v1 authorizations.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadToken {
    pub token: String,
}

/// Query of the download routes. A browser follows the link, so the token travels in the query.
/// Mirrors the `token` param of `getDownloadUrl` and the `Kyros.Files.v1` download URLs.
#[derive(Debug, Clone, Deserialize)]
pub struct DownloadQuery {
    #[serde(default)]
    pub token: Option<String>,
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

/// A backup's name as given, trimmed: 1 to 64 characters.
fn backup_name(name: &str) -> Result<&str, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "A backup name has 1 to 64 characters.",
        ));
    }
    Ok(name)
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

/// The server's schedule and when it next comes due (`None` when scheduled backups are off).
async fn slots(
    state: &AppState,
    id: &str,
) -> Result<(records::Schedule, Option<i64>), ApiError> {
    let schedule = records::schedule(&state.db, id).await?;
    let next = state.backups.next_scheduled(id).await?;
    Ok((schedule, next))
}

/// Every backup with its history, and what is queued or running. Scheduled backups that hold a
/// slot say when they rotate out: the newest after `keep` more scheduled backups, the oldest at
/// the next one.
async fn queue(
    state: &AppState,
    id: &str,
    schedule: &records::Schedule,
    next: Option<i64>,
) -> Result<BackupsQueueResponse, ApiError> {
    let operations = records::operations(&state.db, id).await?;
    let users = users(state, &operations).await?;
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
    let interval = i64::from(schedule.interval_hours) * 3600;
    let mut slotted = 0u32;
    let backups = records::backups(&state.db, id)
        .await?
        .into_iter()
        .map(|backup| {
            let live = backup.state != BackupRowState::Failed;
            let rotates_at = match next {
                Some(next) if backup.kind == Kind::Scheduled && live => {
                    let after = i64::from(
                        schedule.keep.saturating_sub(1).saturating_sub(slotted),
                    );
                    slotted += 1;
                    Some(time(next + after * interval))
                }
                _ => None,
            };
            BackupQueueBackup {
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
                rotates_at,
                history: operations
                    .iter()
                    .filter(|op| op.backup_id == backup.id)
                    .map(|op| {
                        let finished = matches!(
                            op.state,
                            OpState::Done
                                | OpState::Failed
                                | OpState::Cancelled
                        );
                        BackupQueueOperation {
                            operation_type: operation_type(op.kind),
                            operation_id: Some(op.id),
                            state: match op.state {
                                OpState::Queued => BackupQueueState::Pending,
                                OpState::Running => BackupQueueState::Ongoing,
                                OpState::Done => BackupQueueState::Completed,
                                OpState::Failed => BackupQueueState::Failed,
                                OpState::Cancelled => {
                                    BackupQueueState::Cancelled
                                }
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
            }
        })
        .collect();
    Ok(BackupsQueueResponse {
        active_operations,
        backups,
    })
}

/// `GET .../backups-queue`.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<BackupsQueueResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    world(&id, &wid)?;
    let (schedule, next) = slots(&state, &id).await?;
    Ok(Json(queue(&state, &id, &schedule, next).await?))
}

/// `GET .../backups-queue/with-options`: the queue, how the slots are split, and when the next
/// scheduled backup is due.
pub async fn list_with_options(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<BackupsQueueWithOptionsResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    world(&id, &wid)?;
    let (schedule, next) = slots(&state, &id).await?;
    Ok(Json(BackupsQueueWithOptionsResponse {
        queue: queue(&state, &id, &schedule, next).await?,
        options: WorldBackupOptions {
            split_automatic: schedule.keep,
            split_manual: SLOTS.saturating_sub(schedule.keep),
            limit: SLOTS,
            interval: schedule.interval_hours,
        },
        next_auto_backup_at: next.map(time),
    }))
}

/// `PATCH .../backups-queue/with-options`: split the slots and set the interval (hourly at
/// most, monthly at least; no scheduled slots turns scheduled backups off). Editors set it.
/// Fewer scheduled slots than scheduled backups deletes the oldest: without `ar=true` the answer
/// only names them; with it, this is the owner's, with a fresh ticket, as deleting them by hand
/// would be.
pub async fn patch_options(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Query(query): Query<PatchOptionsQuery>,
    headers: HeaderMap,
    Json(body): Json<PatchWorldBackupOptions>,
) -> Result<Json<PatchWorldBackupOptionsResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let current = records::schedule(&state.db, &id).await?;
    let interval_hours = body.interval.unwrap_or(current.interval_hours);
    if !(1..=24 * 31).contains(&interval_hours) {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "Back up at most every hour and at least every 744 hours.",
        ));
    }
    let keep = match (body.split_automatic, body.split_manual) {
        (Some(automatic), Some(manual))
            if automatic.checked_add(manual) != Some(SLOTS) =>
        {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                format!("Scheduled and manual slots add up to {SLOTS}."),
            ));
        }
        (Some(automatic), _) => automatic,
        (None, Some(manual)) => SLOTS.saturating_sub(manual),
        (None, None) => current.keep,
    };
    if keep > SLOTS {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            format!("There are {SLOTS} slots."),
        ));
    }
    let rotated =
        records::excess(&state.db, &id, Kind::Scheduled, keep).await?;
    let acknowledged = query.ar.as_deref() == Some("true");
    if !rotated.is_empty() && !acknowledged {
        return Ok(Json(PatchWorldBackupOptionsResponse {
            applied: false,
            backup_ids: Some(rotated.iter().map(|id| uuid(id)).collect()),
        }));
    }
    let caller = if rotated.is_empty() {
        caller
    } else {
        Dangerous::check(&state, caller, &headers).await?.0
    };
    records::set_schedule(
        &state.db,
        &id,
        records::Schedule {
            interval_hours,
            keep,
            ..current
        },
    )
    .await?;
    for old in &rotated {
        state.backups.delete(&id, old).await?;
        activity::log(
            &state.db,
            &caller,
            Some(&id),
            Action::BackupDeleted {
                backup_id: uuid(old),
            },
        )
        .await;
    }
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(PatchWorldBackupOptionsResponse {
        applied: true,
        backup_ids: (!rotated.is_empty())
            .then(|| rotated.iter().map(|id| uuid(id)).collect()),
    }))
}

/// `POST .../backups-queue`: queue a backup of the server as it is, hot when it runs. Takes a
/// free manual slot.
pub async fn create(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<BackupRequest>,
) -> Result<Json<PostBackupQueueResponse>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let name = backup_name(&request.name)?;
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

/// `PATCH .../backups/{bid}`: rename a backup.
pub async fn rename(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, backup)): Path<(String, String, String)>,
    Json(body): Json<PatchBackup>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let Some(name) = body.name.as_deref() else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let name = backup_name(name)?;
    if !records::rename(&state.db, &id, &backup, name).await? {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            format!("There is no backup {backup}."),
        ));
    }
    activity::log(
        &state.db,
        &caller,
        Some(&id),
        Action::BackupRenamed {
            backup_id: uuid(&backup),
        },
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
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

/// `GET /servers/{id}/v1/worlds/{wid}/backups/{bid}/download/authorize`: a link token for the
/// backup's zip, for a download that cannot send the session.
pub async fn authorize_backup(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid, backup)): Path<(String, String, String)>,
) -> Result<Json<DownloadToken>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
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

/// The answer to a link token that is unknown, used, or past its five minutes.
pub(super) fn expired_link() -> ApiError {
    ApiError::new(
        ErrorCode::Unauthorized,
        "This download link has expired or was used.",
    )
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
            .ok_or_else(expired_link)?,
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

/// `GET /servers/{id}/v1/worlds/{wid}/backups/{bid}/download`: a backup's zip, streamed. The
/// link token goes in `token`; a session in the header works too.
pub async fn download_backup(
    caller: Result<Caller, ApiError>,
    State(state): State<Arc<AppState>>,
    Path((id, wid, backup)): Path<(String, String, String)>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, ApiError> {
    world(&id, &wid)?;
    let target = Target::Backup(backup.clone());
    let caller =
        downloader(&state, caller, query.token.as_deref(), &id, &target)
            .await?;
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
    Query(query): Query<DownloadQuery>,
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
