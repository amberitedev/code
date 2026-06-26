use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use sha2::Digest;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::application::state::{AppState, FsUploadSession};

const MAX_UPLOAD_BYTES: u64 = 1024 * 1024 * 1024 * 8;
const SESSION_TTL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("instance not found")]
    NotFound,
    #[error("upload not found")]
    UploadNotFound,
    #[error("path traversal rejected")]
    PathTraversal,
    #[error("invalid upload: {0}")]
    Invalid(String),
    #[error("offset mismatch")]
    OffsetMismatch,
    #[error("checksum mismatch")]
    ChecksumMismatch,
}

pub struct UploadSessionView {
    pub id: String,
    pub offset: u64,
    pub length: u64,
}

pub async fn create_upload_session(
    state: &Arc<AppState>,
    instance_id: &str,
    destination_path: &str,
    length: u64,
    sha256: Option<String>,
) -> Result<UploadSessionView, UploadError> {
    if length == 0 || length > MAX_UPLOAD_BYTES {
        return Err(UploadError::Invalid(format!(
            "upload length must be 1-{MAX_UPLOAD_BYTES} bytes"
        )));
    }
    if let Some(sha256) = &sha256 {
        if sha256.len() != 64
            || !sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(UploadError::Invalid(
                "sha256 metadata must be a hex digest".into(),
            ));
        }
    }
    let data_dir = instance_data_dir(state, instance_id).await?;
    let destination = guarded_destination(&data_dir, destination_path).await?;
    let id = Uuid::new_v4().to_string();
    let partial_dir = state.config.data_dir.join("tmp").join("uploads");
    tokio::fs::create_dir_all(&partial_dir).await?;
    let partial_path = partial_dir.join(format!("{id}.part"));
    tokio::fs::File::create(&partial_path).await?;
    state.fs_upload_sessions.insert(
        id.clone(),
        FsUploadSession {
            instance_id: instance_id.to_string(),
            destination,
            partial_path,
            length,
            offset: 0,
            sha256,
            expires_at: Instant::now() + SESSION_TTL,
        },
    );
    Ok(UploadSessionView {
        id,
        offset: 0,
        length,
    })
}

pub fn upload_status(
    state: &Arc<AppState>,
    instance_id: &str,
    upload_id: &str,
) -> Result<UploadSessionView, UploadError> {
    let session = state
        .fs_upload_sessions
        .get(upload_id)
        .ok_or(UploadError::UploadNotFound)?;
    if session.instance_id != instance_id {
        return Err(UploadError::UploadNotFound);
    }
    Ok(UploadSessionView {
        id: upload_id.to_string(),
        offset: session.offset,
        length: session.length,
    })
}

pub async fn append_upload(
    state: &Arc<AppState>,
    instance_id: &str,
    upload_id: &str,
    expected_offset: u64,
    chunk: Bytes,
    chunk_sha256: Option<String>,
) -> Result<UploadSessionView, UploadError> {
    let mut session = state
        .fs_upload_sessions
        .get_mut(upload_id)
        .ok_or(UploadError::UploadNotFound)?;
    if session.instance_id != instance_id {
        return Err(UploadError::UploadNotFound);
    }
    if session.offset != expected_offset {
        return Err(UploadError::OffsetMismatch);
    }
    if let Some(expected) = chunk_sha256 {
        let actual = hex::encode(sha2::Sha256::digest(&chunk));
        if actual != expected {
            return Err(UploadError::ChecksumMismatch);
        }
    }
    let chunk_len = chunk.len() as u64;
    let new_offset = session
        .offset
        .checked_add(chunk_len)
        .ok_or_else(|| UploadError::Invalid("upload offset overflow".into()))?;
    if new_offset > session.length {
        return Err(UploadError::Invalid(
            "chunk exceeds declared upload length".into(),
        ));
    }
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(&session.partial_path)
        .await?;
    file.write_all(&chunk).await?;
    session.offset = new_offset;
    session.expires_at = Instant::now() + SESSION_TTL;
    let view = UploadSessionView {
        id: upload_id.to_string(),
        offset: session.offset,
        length: session.length,
    };
    if session.offset == session.length {
        let finalized = session.clone();
        drop(session);
        finalize_upload(state, upload_id, finalized).await?;
    }
    Ok(view)
}

pub async fn cancel_upload(
    state: &Arc<AppState>,
    instance_id: &str,
    upload_id: &str,
) -> Result<(), UploadError> {
    let Some((_, session)) = state.fs_upload_sessions.remove(upload_id) else {
        return Err(UploadError::UploadNotFound);
    };
    if session.instance_id != instance_id {
        state
            .fs_upload_sessions
            .insert(upload_id.to_string(), session);
        return Err(UploadError::UploadNotFound);
    }
    tokio::fs::remove_file(session.partial_path).await.ok();
    Ok(())
}

async fn finalize_upload(
    state: &Arc<AppState>,
    upload_id: &str,
    session: FsUploadSession,
) -> Result<(), UploadError> {
    if let Some(expected) = &session.sha256 {
        let bytes = tokio::fs::read(&session.partial_path).await?;
        let actual = hex::encode(sha2::Sha256::digest(&bytes));
        if &actual != expected {
            state.fs_upload_sessions.remove(upload_id);
            tokio::fs::remove_file(&session.partial_path).await.ok();
            return Err(UploadError::ChecksumMismatch);
        }
    }
    if let Some(parent) = session.destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    ensure_no_destination_symlink(&session.destination)?;
    tokio::fs::rename(&session.partial_path, &session.destination).await?;
    state.fs_upload_sessions.remove(upload_id);
    Ok(())
}

async fn instance_data_dir(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<PathBuf, UploadError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT data_dir FROM instances WHERE id = ?")
            .bind(instance_id)
            .fetch_optional(&state.pool)
            .await?;
    let (dir,) = row.ok_or(UploadError::NotFound)?;
    Ok(PathBuf::from(dir))
}

async fn guarded_destination(
    data_dir: &Path,
    client_path: &str,
) -> Result<PathBuf, UploadError> {
    let trimmed = client_path.trim_start_matches('/');
    if trimmed.is_empty() || trimmed.ends_with('/') || trimmed.ends_with('\\') {
        return Err(UploadError::Invalid("path must name a file".into()));
    }
    let rel = Path::new(trimmed);
    for component in rel.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(UploadError::PathTraversal);
        }
    }
    let destination = data_dir.join(rel);
    let parent = destination.parent().unwrap_or(data_dir);
    tokio::fs::create_dir_all(parent).await?;
    let canonical_base = data_dir.canonicalize()?;
    let canonical_parent = parent.canonicalize()?;
    if !canonical_parent.starts_with(canonical_base) {
        return Err(UploadError::PathTraversal);
    }
    Ok(destination)
}

fn ensure_no_destination_symlink(path: &Path) -> Result<(), UploadError> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(UploadError::PathTraversal)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(UploadError::Io(error)),
    }
}
