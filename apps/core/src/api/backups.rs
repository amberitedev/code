//! A server's backups: the list and its schedule, creating, renaming, restoring and deleting
//! them, and their operations. Also the downloads, which a native save follows with a link
//! (`access::links`): `GET /servers/{id}/v1/worlds/{wid}/backups/{bid}/download` and
//! `.../files/download-full-zip`.
//!
//! Core has [`SLOTS`] backup slots per server, split between scheduled and manual backups; the
//! schedule's `keep` is the scheduled share.
//!
//! Viewers see the list and the schedule. Editors create, rename, and download backups and the
//! world, and set the schedule. Restoring and deleting are the owner's, with a fresh ticket, and
//! so is keeping fewer scheduled backups than there are, which deletes the oldest.

use std::{collections::HashMap, sync::Arc};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    core::{
        AckOperation, Action, BackupKind, BackupOperation, BackupSchedule,
        BackupState, CancelOperation, ChangeBackupSchedule, CoreUser,
        CreateBackup, CreatedBackup, DeleteBackup, DeleteBackups, ListBackups,
        OperationKind, OperationState, RenameBackup, RestoreBackup,
        RetryBackup, ScheduleChange, ServerBackup, ServerBackups,
    },
    server::Req,
};
use uuid::Uuid;

use super::{CoreState, content::world, internal};
use crate::{
    access::{Caller, Dangerous, Permission, activity},
    backups::{
        self, SLOTS,
        records::{self, Kind, OpKind, OpState, State as BackupRowState},
    },
    servers::ServerError,
    state::servers::adapters::sqlite::server_rows,
};

impl From<backups::Error> for ApiError {
    fn from(error: backups::Error) -> Self {
        use backups::Error;
        match error {
            Error::Server(error) => error.into(),
            error => {
                let code = match &error {
                    Error::NotFound(_) => ErrorCode::NotFound,
                    Error::Conflict(_) | Error::Space(_) => ErrorCode::Conflict,
                    _ => ErrorCode::InternalError,
                };
                Self::new(code, format!("{error:#}"))
            }
        }
    }
}

fn uuid(id: &str) -> Uuid {
    Uuid::parse_str(id).unwrap_or_default()
}

/// A backup's name as given, trimmed: 1 to 64 characters.
fn backup_name(name: &str) -> Result<&str, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "A backup name has 1 to 64 characters.",
        ));
    }
    Ok(name)
}

/// Who started each operation, by user id, as the last ticket for them named them.
async fn users(
    state: &CoreState,
    operations: &[records::Operation],
) -> Result<HashMap<String, CoreUser>, ApiError> {
    let mut users = HashMap::new();
    for user_id in operations.iter().filter_map(|op| op.user_id.as_ref()) {
        if users.contains_key(user_id) {
            continue;
        }
        let row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT name, avatar_url FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&state.db)
                .await
                .map_err(internal)?;
        let (name, avatar_url) = row.unwrap_or_else(|| (user_id.clone(), None));
        users.insert(
            user_id.clone(),
            CoreUser {
                id: user_id.clone(),
                name,
                avatar_url,
            },
        );
    }
    Ok(users)
}

fn operation_kind(kind: OpKind) -> OperationKind {
    match kind {
        OpKind::Create => OperationKind::Create,
        OpKind::Restore => OperationKind::Restore,
    }
}

fn operation_state(state: OpState) -> OperationState {
    match state {
        OpState::Queued => OperationState::Queued,
        OpState::Running => OperationState::Running,
        OpState::Done => OperationState::Done,
        OpState::Failed => OperationState::Failed,
        OpState::Cancelled => OperationState::Cancelled,
    }
}

/// Every backup with its operations, and the schedule. Scheduled backups that hold a slot say
/// when they rotate out: the newest after `keep` more scheduled backups, the oldest at the next
/// one.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<ListBackups>,
) -> Result<Json<ServerBackups>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::View).await?;
    let schedule = records::schedule(&state.db, &id).await.map_err(internal)?;
    let next = state.backups.next_scheduled(&id).await?;
    let operations = records::operations(&state.db, &id)
        .await
        .map_err(internal)?;
    let users = users(&state, &operations).await?;
    let interval = i64::from(schedule.interval_hours) * 3600;
    let mut slotted = 0u32;
    let backups = records::backups(&state.db, &id)
        .await
        .map_err(internal)?
        .into_iter()
        .map(|backup| {
            let live = backup.state != BackupRowState::Failed;
            let rotates_at = match next {
                Some(next) if backup.kind == Kind::Scheduled && live => {
                    let after = i64::from(
                        schedule.keep.saturating_sub(1).saturating_sub(slotted),
                    );
                    slotted += 1;
                    Some(next + after * interval)
                }
                _ => None,
            };
            let operations = operations
                .iter()
                .filter(|op| op.backup_id == backup.id)
                .map(|op| BackupOperation {
                    id: op.id,
                    kind: operation_kind(op.kind),
                    state: operation_state(op.state),
                    queued_at: op.queued_at,
                    started_at: op.started_at,
                    ended_at: op.ended_at,
                    error: op.error.clone(),
                    acked: op.acked,
                    user: op
                        .user_id
                        .as_ref()
                        .and_then(|user| users.get(user))
                        .cloned(),
                })
                .collect();
            ServerBackup {
                id: backup.id,
                name: backup.name,
                created_at: backup.created_at,
                state: match backup.state {
                    BackupRowState::Queued => BackupState::Queued,
                    BackupRowState::Running => BackupState::Running,
                    BackupRowState::Done => BackupState::Done,
                    BackupRowState::Failed => BackupState::Failed,
                },
                kind: match backup.kind {
                    Kind::Manual => BackupKind::Manual,
                    Kind::Scheduled => BackupKind::Scheduled,
                    Kind::PreUpdate => BackupKind::PreUpdate,
                },
                rotates_at,
                operations,
            }
        })
        .collect();
    Ok(Json(ServerBackups {
        backups,
        schedule: BackupSchedule {
            interval_hours: schedule.interval_hours,
            keep: schedule.keep,
            slots: SLOTS,
        },
        next_scheduled_at: next,
    }))
}

/// Set the interval (hourly at most, monthly at least) and how many slots scheduled backups keep
/// (none turns them off). Keeping fewer than there are deletes the oldest: without
/// `delete_excess` the answer only names them; with it, this is the owner's, with a fresh ticket,
/// as deleting them by hand would be.
pub async fn change_schedule(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    headers: HeaderMap,
    Req { params, body, .. }: Req<ChangeBackupSchedule>,
) -> Result<Json<ScheduleChange>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let current = records::schedule(&state.db, &id).await.map_err(internal)?;
    let interval_hours = body.interval_hours.unwrap_or(current.interval_hours);
    if !(1..=24 * 31).contains(&interval_hours) {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "Back up at most every hour and at least every 744 hours.",
        ));
    }
    let keep = body.keep.unwrap_or(current.keep);
    if keep > SLOTS {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            format!("There are {SLOTS} slots."),
        ));
    }
    let excess = records::excess(&state.db, &id, Kind::Scheduled, keep)
        .await
        .map_err(internal)?;
    if !excess.is_empty() && !body.delete_excess {
        return Ok(Json(ScheduleChange {
            applied: false,
            excess,
        }));
    }
    let caller = if excess.is_empty() {
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
    .await
    .map_err(internal)?;
    for old in &excess {
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
    Ok(Json(ScheduleChange {
        applied: true,
        excess,
    }))
}

pub async fn create(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<CreateBackup>,
) -> Result<Json<CreatedBackup>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let name = backup_name(&body.name)?;
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
    Ok(Json(CreatedBackup { id: backup }))
}

pub async fn rename(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<RenameBackup>,
) -> Result<Empty, ApiError> {
    let (id, backup) = (params.id, params.backup_id);
    state.access.require(&caller, &id, Permission::Edit).await?;
    let name = backup_name(&body.name)?;
    if !records::rename(&state.db, &id, &backup, name)
        .await
        .map_err(internal)?
    {
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
    Ok(Empty)
}

pub async fn delete(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<DeleteBackup>,
) -> Result<Empty, ApiError> {
    let (id, backup) = (params.id, params.backup_id);
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
    Ok(Empty)
}

pub async fn delete_many(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<DeleteBackups>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    for backup in &body.ids {
        state.backups.delete(&id, backup).await?;
        activity::log(
            &state.db,
            &caller,
            Some(&id),
            Action::BackupDeleted {
                backup_id: uuid(backup),
            },
        )
        .await;
    }
    Ok(Empty)
}

/// Queued, so it answers before the restore is done.
pub async fn restore(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<RestoreBackup>,
) -> Result<StatusCode, ApiError> {
    let (id, backup) = (params.id, params.backup_id);
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

pub async fn retry(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<RetryBackup>,
) -> Result<StatusCode, ApiError> {
    let (id, backup) = (params.id, params.backup_id);
    state.access.require(&caller, &id, Permission::Edit).await?;
    state.backups.retry(&id, &backup, caller.user_id()).await?;
    Ok(StatusCode::ACCEPTED)
}

pub async fn ack(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<AckOperation>,
) -> Result<Empty, ApiError> {
    operation(&state, &caller, &params.id, params.operation_id, false).await
}

pub async fn cancel(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<CancelOperation>,
) -> Result<Empty, ApiError> {
    operation(&state, &caller, &params.id, params.operation_id, true).await
}

/// Stop showing an operation that ended, or drop a queued one (a cancelled create takes its
/// backup with it).
async fn operation(
    state: &CoreState,
    caller: &Caller,
    id: &str,
    operation: i64,
    cancel: bool,
) -> Result<Empty, ApiError> {
    state.access.require(caller, id, Permission::Edit).await?;
    let found = records::operation(&state.db, operation)
        .await
        .map_err(internal)?
        .filter(|op| op.server_id == id);
    if found.is_none() {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "There is no such operation.",
        ));
    }
    let done = if cancel {
        records::cancel(&state.db, id, operation).await
    } else {
        records::ack(&state.db, id, operation).await
    }
    .map_err(internal)?;
    if !done {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            if cancel {
                "Only a queued operation can be cancelled."
            } else {
                "The operation has not finished."
            },
        ));
    }
    Ok(Empty)
}

/// A zip streamed to the client, saved as `name` with `extension`. A `temporary` file is removed
/// once the response is done with it.
pub(super) fn zip_response(
    file: std::fs::File,
    name: &str,
    extension: &str,
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
                format!("attachment; filename=\"{name}.{extension}\""),
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

/// `GET /servers/{id}/v1/worlds/{wid}/backups/{bid}/download`: a backup's zip, streamed.
pub async fn download_backup(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Path((id, wid, backup)): Path<(String, String, String)>,
) -> Result<Response, ApiError> {
    world(&id, &wid)?;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let found = records::backup(&state.db, &id, &backup)
        .await
        .map_err(internal)?
        .filter(|found| found.state == BackupRowState::Done)
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::NotFound,
                format!("There is no finished backup {backup}."),
            )
        })?;
    let file = std::fs::File::open(state.backups.file(&id, &backup))
        .map_err(internal)?;
    activity::log(
        &state.db,
        &caller,
        Some(&id),
        Action::BackupDownloaded {
            backup_id: uuid(&backup),
        },
    )
    .await;
    Ok(zip_response(file, &found.name, "zip", None))
}

/// `GET /servers/{id}/v1/worlds/{wid}/files/download-full-zip`: the world and settings as they
/// are now, copied as a backup is (hot when the server runs) and streamed, then removed.
pub async fn download_world(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    world(&id, &wid)?;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let server = server_rows::get(&state.db, &id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let path = state
        .servers
        .paths()
        .backups()
        .join(&id)
        .join(format!("download-{}.zip", Uuid::new_v4()));
    state.backups.snapshot(&id, &path, |_| {}).await?;
    let file = std::fs::File::open(&path).map_err(internal)?;
    activity::log(&state.db, &caller, Some(&id), Action::WorldDownloaded).await;
    Ok(zip_response(file, &server.name, "zip", Some(path)))
}
