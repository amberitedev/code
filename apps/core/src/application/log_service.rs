use std::{path::PathBuf, sync::Arc};

use serde::Serialize;

use crate::application::state::AppState;

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub filename: String,
    pub size_bytes: u64,
    pub modified_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("db: {0}")] Db(#[from] sqlx::Error),
    #[error("not found")] NotFound,
    #[error("invalid path")] InvalidPath,
}

async fn instance_data_dir(state: &Arc<AppState>, instance_id: &str) -> Result<PathBuf, LogError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT data_dir FROM instances WHERE id = ?")
        .bind(instance_id)
        .fetch_optional(&state.pool)
        .await?;
    let (dir,) = row.ok_or(LogError::NotFound)?;
    Ok(PathBuf::from(dir))
}

async fn scan_dir(dir: PathBuf, extensions: &[&str]) -> Vec<LogEntry> {
    let mut entries = vec![];
    let mut rd = match tokio::fs::read_dir(&dir).await {
        Ok(r) => r,
        Err(_) => return entries,
    };
    while let Ok(Some(e)) = rd.next_entry().await {
        let name = e.file_name().to_string_lossy().to_string();
        if !extensions.iter().any(|ext| name.ends_with(ext)) {
            continue;
        }
        if let Ok(meta) = e.metadata().await {
            let modified = meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                .unwrap_or_default();
            entries.push(LogEntry {
                filename: name,
                size_bytes: meta.len(),
                modified_at: modified,
            });
        }
    }
    entries
}

pub async fn list_logs(state: &Arc<AppState>, instance_id: &str) -> Result<Vec<LogEntry>, LogError> {
    let dir = instance_data_dir(state, instance_id).await?;
    Ok(scan_dir(dir.join("logs"), &[".log", ".log.gz"]).await)
}

/// Resolve a log filename to its path; returns `(path, is_gzipped)`.
pub async fn resolve_log(
    state: &Arc<AppState>,
    instance_id: &str,
    filename: &str,
) -> Result<(PathBuf, bool), LogError> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(LogError::InvalidPath);
    }
    if !filename.ends_with(".log") && !filename.ends_with(".log.gz") {
        return Err(LogError::InvalidPath);
    }
    let dir = instance_data_dir(state, instance_id).await?;
    let path = dir.join("logs").join(filename);
    if !path.exists() {
        return Err(LogError::NotFound);
    }
    Ok((path, filename.ends_with(".gz")))
}

pub async fn list_crash_reports(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<Vec<LogEntry>, LogError> {
    let dir = instance_data_dir(state, instance_id).await?;
    Ok(scan_dir(dir.join("crash-reports"), &[".txt"]).await)
}

pub async fn resolve_crash(
    state: &Arc<AppState>,
    instance_id: &str,
    filename: &str,
) -> Result<PathBuf, LogError> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(LogError::InvalidPath);
    }
    if !filename.ends_with(".txt") {
        return Err(LogError::InvalidPath);
    }
    let dir = instance_data_dir(state, instance_id).await?;
    let path = dir.join("crash-reports").join(filename);
    if !path.exists() {
        return Err(LogError::NotFound);
    }
    Ok(path)
}
