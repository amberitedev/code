//! Upload sessions: files staged outside the instance, one per request, then put in place
//! together. Sessions live in memory, so an engine restart ends them.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::body::Body;
use contracts::{ApiError, ErrorCode};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

use super::sandboxed::{
    changing, inner_path, internal, locked, place, refuse_owned, staging,
};
use crate::{
    Engine,
    access::Caller,
    archive::{self, NoRoom},
    endpoints::files::{InstanceUpload, InstanceUploadStatus, UploadScope},
    routes::api_error,
    sandbox,
};

/// How long an upload session lives without a request.
const SESSION_IDLE_S: i64 = 3600;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

struct Session {
    instance_id: String,
    scope: UploadScope,
    /// Who opened it; `None` for the host's own caller.
    user_id: Option<String>,
    status: InstanceUploadStatus,
    created_at: i64,
    updated_at: i64,
    last_upload_at: Option<i64>,
    /// Where its files wait. Removed whenever the session ends.
    dir: PathBuf,
    /// The name each file was sent with, and where it waits.
    files: Vec<(String, PathBuf)>,
    bytes: u64,
    /// Held by the upload in progress: uploads to one session take turns, so each streams
    /// against what the ones before it left of the disk, not all against the same remainder.
    turn: Arc<tokio::sync::Mutex<()>>,
}

impl Session {
    fn expires_at(&self) -> i64 {
        self.updated_at + SESSION_IDLE_S
    }

    fn response(&self, upload_id: &str) -> InstanceUpload {
        InstanceUpload {
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
            InstanceUploadStatus::Active | InstanceUploadStatus::Uploading
        )
    }

    fn belongs_to(
        &self,
        caller: &Caller,
        instance_id: &str,
        scope: Option<UploadScope>,
    ) -> bool {
        self.instance_id == instance_id
            && scope.is_none_or(|scope| self.scope == scope)
            && self.user_id.as_deref() == caller.user_id()
    }

    /// End the session: it is over and holds no files.
    fn end(&mut self, status: InstanceUploadStatus) {
        self.status = status;
        self.updated_at = now();
        self.files.clear();
    }
}

/// Every upload session, by id.
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
        instance_id: &str,
        upload_id: &str,
        change: impl FnOnce(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        let mut sessions = self.lock();
        let session = sessions
            .get_mut(upload_id)
            .filter(|session| session.belongs_to(caller, instance_id, None))
            .ok_or_else(|| {
                ApiError::new(ErrorCode::NotFound, "There is no such upload.")
            })?;
        if !session.open() {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "This upload is over.",
            ));
        }
        change(session)
    }
}

pub(super) async fn open(
    engine: &Engine,
    caller: &Caller,
    instance_id: &str,
    scope: UploadScope,
) -> InstanceUpload {
    let upload_id = uuid::Uuid::new_v4().to_string();
    let now = now();
    let session = Session {
        instance_id: instance_id.to_owned(),
        scope,
        user_id: caller.user_id().map(str::to_owned),
        status: InstanceUploadStatus::Active,
        created_at: now,
        updated_at: now,
        last_upload_at: None,
        dir: staging(engine).join(format!("upload-{upload_id}")),
        files: Vec::new(),
        bytes: 0,
        turn: Arc::default(),
    };
    let response = session.response(&upload_id);
    let ended: Vec<PathBuf> = {
        let mut sessions = engine.uploads.lock();
        let ended = sessions
            .values_mut()
            .filter(|other| {
                other.open()
                    && other.belongs_to(caller, instance_id, Some(scope))
            })
            .map(|other| {
                other.end(InstanceUploadStatus::Cancelled);
                other.dir.clone()
            })
            .collect();
        sessions.insert(upload_id, session);
        ended
    };
    for dir in ended {
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
    response
}

pub(super) fn get(
    engine: &Engine,
    caller: &Caller,
    instance_id: &str,
    scope: UploadScope,
) -> Option<InstanceUpload> {
    engine
        .uploads
        .lock()
        .iter()
        .find(|(_, session)| {
            session.open()
                && session.belongs_to(caller, instance_id, Some(scope))
        })
        .map(|(upload_id, session)| session.response(upload_id))
}

/// Stream `body` into `staged`, as much as fits on the disk (`archive::room`).
async fn stage(body: Body, staged: &std::path::Path) -> Result<u64, ApiError> {
    let parent = staged.parent().unwrap_or(staged);
    tokio::fs::create_dir_all(parent).await.map_err(internal)?;
    let room = archive::room(parent).map_err(internal)?;
    let result = async {
        let mut file =
            tokio::fs::File::create(staged).await.map_err(internal)?;
        let mut stream = body.into_data_stream();
        let mut written = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| {
                ApiError::new(ErrorCode::InvalidInput, error.to_string())
            })?;
            written += chunk.len() as u64;
            if written > room {
                return Err(ApiError::new(
                    ErrorCode::Conflict,
                    NoRoom {
                        what: "the upload".into(),
                        needed: None,
                        room,
                    }
                    .to_string(),
                ));
            }
            file.write_all(&chunk).await.map_err(internal)?;
        }
        file.flush().await.map_err(internal)?;
        Ok(written)
    }
    .await;
    // Whatever stopped it, a client gone or a full disk, nothing half-written stays.
    if result.is_err() {
        let _ = tokio::fs::remove_file(staged).await;
    }
    result
}

/// Stage `body` as the file `name`: a path in the instance for `files`, a file name for
/// `content`. It counts only if the session is still open once all of it has arrived.
pub(super) async fn add_file(
    engine: &Engine,
    caller: &Caller,
    instance_id: &str,
    upload_id: &str,
    name: &str,
    body: Body,
) -> Result<InstanceUpload, ApiError> {
    let (turn, scope) =
        engine
            .uploads
            .change(caller, instance_id, upload_id, |session| {
                Ok((Arc::clone(&session.turn), session.scope))
            })?;
    // Checked now, so a bad name fails before it is stored.
    let file_name = match scope {
        UploadScope::Files => inner_path(name)?,
        UploadScope::Content => sandbox::clean(name).map_err(|error| {
            ApiError::new(ErrorCode::InvalidInput, error.to_string())
        })?,
    }
    .rsplit('/')
    .next()
    .unwrap_or_default()
    .to_owned();
    let _turn = turn.lock_owned().await;
    // Read once it is this upload's turn: what the uploads before it committed counts.
    let dir =
        engine
            .uploads
            .change(caller, instance_id, upload_id, |session| {
                Ok(session.dir.clone())
            })?;
    // Its own folder keeps the name, which content reads its kind from.
    let staged = dir.join(uuid::Uuid::new_v4().to_string()).join(file_name);
    let size = stage(body, &staged).await?;
    let committed =
        engine
            .uploads
            .change(caller, instance_id, upload_id, |session| {
                let now = now();
                session.files.push((name.to_owned(), staged.clone()));
                session.bytes += size;
                session.status = InstanceUploadStatus::Uploading;
                session.updated_at = now;
                session.last_upload_at = Some(now);
                Ok(session.response(upload_id))
            });
    if committed.is_err() {
        let _ = tokio::fs::remove_file(&staged).await;
    }
    committed
}

/// Put every staged file in place. Files go where their names say, never over a server-owned
/// file; content goes through the instance's content.
pub(super) async fn finalize(
    engine: &Engine,
    caller: &Caller,
    instance_id: &str,
    upload_id: &str,
) -> Result<InstanceUpload, ApiError> {
    // Taken in one step, so a second finalize finds the session over.
    let (scope, files, dir) =
        engine
            .uploads
            .change(caller, instance_id, upload_id, |session| {
                session.status = InstanceUploadStatus::Finalizing;
                session.updated_at = now();
                Ok((session.scope, session.files.clone(), session.dir.clone()))
            })?;
    let result = async {
        match scope {
            UploadScope::Files => {
                let files = files
                    .iter()
                    .map(|(name, staged)| {
                        Ok((inner_path(name)?, staged.clone()))
                    })
                    .collect::<Result<Vec<_>, ApiError>>()?;
                let (folder, lock) = changing(engine, instance_id).await?;
                locked(lock, move || {
                    for (path, _) in &files {
                        refuse_owned(&folder, path)?;
                    }
                    for (path, staged) in &files {
                        place(&folder, path, staged)?;
                    }
                    Ok(())
                })
                .await
            }
            UploadScope::Content => {
                for (_, staged) in &files {
                    crate::instance::add_project_from_path(
                        engine,
                        instance_id,
                        staged,
                        None,
                    )
                    .await
                    .map_err(api_error)?;
                }
                Ok(())
            }
        }
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    let status = if result.is_ok() {
        InstanceUploadStatus::Finalized
    } else {
        InstanceUploadStatus::Cancelled
    };
    let response = engine.uploads.lock().get_mut(upload_id).map(|session| {
        let bytes = session.bytes;
        let count = session.files.len();
        session.end(status);
        let mut response = session.response(upload_id);
        response.entry_count = count as u32;
        response.uploaded_byte_count = bytes;
        response
    });
    result?;
    response.ok_or_else(|| {
        ApiError::new(ErrorCode::NotFound, "There is no such upload.")
    })
}

/// Cancel it and remove what it staged. A session being finalized can no longer be cancelled.
pub(super) async fn cancel(
    engine: &Engine,
    caller: &Caller,
    instance_id: &str,
    upload_id: &str,
) -> Result<InstanceUpload, ApiError> {
    let (response, dir) =
        engine
            .uploads
            .change(caller, instance_id, upload_id, |session| {
                session.end(InstanceUploadStatus::Cancelled);
                Ok((session.response(upload_id), session.dir.clone()))
            })?;
    let _ = tokio::fs::remove_dir_all(dir).await;
    Ok(response)
}
