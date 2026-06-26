use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    application::{rcon_service, state::AppState, task_service::validate_cron},
    domain::instance::{InstanceId, InstanceStatus},
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BackupRecord {
    pub id: String,
    pub instance_id: String,
    pub name: String,
    pub size_bytes: i64,
    pub locked: bool,
    pub trigger: String,
    pub hot: bool,
    pub consistency: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct BackupSchedule {
    pub enabled: bool,
    pub cron: String,
    pub retain_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct BackupTaskPayload {
    retain_count: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("zip: {0}")]
    Zip(String),
    #[error("instance not found")]
    NotFound,
    #[error("backup locked")]
    Locked,
    #[error("instance must be offline to restore")]
    MustBeOffline,
    #[error("path traversal rejected")]
    PathTraversal,
    #[error("rcon_required_for_hot_backup")]
    RconRequiredForHotBackup,
    #[error("rcon: {0}")]
    Rcon(String),
    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),
}

async fn data_dir_for(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<PathBuf, BackupError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT data_dir FROM instances WHERE id = ?")
            .bind(instance_id)
            .fetch_optional(&state.pool)
            .await?;
    let (dir,) = row.ok_or(BackupError::NotFound)?;
    Ok(PathBuf::from(dir))
}

pub(crate) fn storage_dir(state: &Arc<AppState>, instance_id: &str) -> PathBuf {
    state.config.data_dir.join("backups").join(instance_id)
}

fn zip_data_dir(data_dir: &Path, zip_path: &Path) -> Result<u64, BackupError> {
    use zip::{write::SimpleFileOptions, ZipWriter};

    let file = std::fs::File::create(zip_path).map_err(BackupError::Io)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let excluded = ["logs", "crash-reports"];

    for entry in walkdir::WalkDir::new(data_dir).min_depth(1) {
        let entry = entry.map_err(|e| {
            let msg = e.to_string();
            BackupError::Io(
                e.into_io_error()
                    .unwrap_or_else(|| std::io::Error::other(msg)),
            )
        })?;
        if entry.file_type().is_symlink() {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(data_dir).unwrap();
        if let Some(first) = rel.components().next() {
            if excluded.iter().any(|ex| first.as_os_str() == *ex) {
                continue;
            }
        }
        let name = rel.to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            zip.add_directory(&name, options)
                .map_err(|e| BackupError::Zip(e.to_string()))?;
        } else {
            zip.start_file(&name, options)
                .map_err(|e| BackupError::Zip(e.to_string()))?;
            let mut f = std::fs::File::open(path).map_err(BackupError::Io)?;
            std::io::copy(&mut f, &mut zip).map_err(BackupError::Io)?;
        }
    }
    zip.finish().map_err(|e| BackupError::Zip(e.to_string()))?;
    Ok(std::fs::metadata(zip_path)?.len())
}

fn unzip_to(zip_path: &Path, target: &Path) -> Result<(), BackupError> {
    let file = std::fs::File::open(zip_path).map_err(BackupError::Io)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| BackupError::Zip(e.to_string()))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| BackupError::Zip(e.to_string()))?;
        if entry
            .unix_mode()
            .map(|mode| mode & 0o170000 == 0o120000)
            .unwrap_or(false)
        {
            return Err(BackupError::PathTraversal);
        }
        let out = guarded_archive_path(target, entry.name())?;
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(BackupError::Io)?;
        } else {
            guard_parent(target, &out)?;
            let mut f = std::fs::File::create(&out).map_err(BackupError::Io)?;
            std::io::copy(&mut entry, &mut f).map_err(BackupError::Io)?;
        }
    }
    Ok(())
}

fn guarded_archive_path(
    base: &Path,
    entry_name: &str,
) -> Result<PathBuf, BackupError> {
    let rel = Path::new(entry_name);
    for component in rel.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(BackupError::PathTraversal);
        }
    }
    Ok(base.join(rel))
}

fn guard_parent(base: &Path, path: &Path) -> Result<(), BackupError> {
    let parent = path.parent().unwrap_or(base);
    std::fs::create_dir_all(parent).map_err(BackupError::Io)?;
    let canonical_base = base.canonicalize().map_err(BackupError::Io)?;
    let canonical_parent = parent.canonicalize().map_err(BackupError::Io)?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(BackupError::PathTraversal);
    }
    Ok(())
}

pub async fn list_backups(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<Vec<BackupRecord>, BackupError> {
    let rows = sqlx::query_as::<_, BackupRecord>(
		"SELECT id, instance_id, name, size_bytes, locked, trigger, hot, consistency, created_at FROM backups WHERE instance_id = ? ORDER BY created_at DESC",
	)
	.bind(instance_id)
	.fetch_all(&state.pool)
	.await?;
    Ok(rows)
}

pub async fn create_backup(
    state: &Arc<AppState>,
    instance_id: &str,
    trigger: &str,
    name: Option<String>,
) -> Result<BackupRecord, BackupError> {
    let iid: InstanceId =
        instance_id.parse().map_err(|_| BackupError::NotFound)?;
    let record = state
        .instance_store
        .get(&iid)
        .await
        .map_err(|_| BackupError::NotFound)?;
    let data_dir = PathBuf::from(&record.data_dir);
    let running = record.status == InstanceStatus::Running
        || state.instances.contains_key(&iid);
    let trigger = normalize_trigger(trigger);
    let store_dir = storage_dir(state, instance_id);
    let id = Uuid::new_v4().to_string();
    let zip_path = store_dir.join(format!("{id}.zip"));
    let display_name = name.unwrap_or_else(|| {
        chrono::Utc::now()
            .format("backup-%Y%m%d-%H%M%S")
            .to_string()
    });
    let (hot, consistency, size_bytes) = if running {
        run_rcon_for_backup(state, instance_id, "save-off").await?;
        let result = async {
            run_rcon_for_backup(state, instance_id, "save-all flush").await?;
            write_backup_archive(&data_dir, &zip_path, &store_dir).await
        }
        .await;
        if let Err(error) =
            run_rcon_for_backup(state, instance_id, "save-on").await
        {
            tracing::warn!(
                instance_id,
                error = %error,
                "failed to re-enable world saves after hot backup"
            );
        }
        (true, "rcon_flush".to_string(), result?)
    } else {
        (
            false,
            "offline".to_string(),
            write_backup_archive(&data_dir, &zip_path, &store_dir).await?,
        )
    };

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
		"INSERT INTO backups (id, instance_id, name, size_bytes, locked, trigger, hot, consistency, created_at) VALUES (?, ?, ?, ?, 0, ?, ?, ?, ?)",
	)
	.bind(&id)
	.bind(instance_id)
	.bind(&display_name)
	.bind(size_bytes as i64)
	.bind(&trigger)
	.bind(hot)
	.bind(&consistency)
	.bind(&now)
	.execute(&state.pool)
	.await?;

    Ok(BackupRecord {
        id,
        instance_id: instance_id.to_string(),
        name: display_name,
        size_bytes: size_bytes as i64,
        locked: false,
        trigger,
        hot,
        consistency,
        created_at: now,
    })
}

fn normalize_trigger(trigger: &str) -> String {
    match trigger {
        "automatic" | "automated" | "scheduled" => "scheduled".to_string(),
        _ => "manual".to_string(),
    }
}

async fn write_backup_archive(
    data_dir: &Path,
    zip_path: &Path,
    store_dir: &Path,
) -> Result<u64, BackupError> {
    let data_dir_c = data_dir.to_path_buf();
    let zip_path_c = zip_path.to_path_buf();
    let store_dir_c = store_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&store_dir_c)?;
        zip_data_dir(&data_dir_c, &zip_path_c)
    })
    .await
    .map_err(|e| BackupError::Zip(e.to_string()))?
}

async fn run_rcon_for_backup(
    state: &Arc<AppState>,
    instance_id: &str,
    command: &str,
) -> Result<(), BackupError> {
    rcon_service::execute_command(state, instance_id, command)
        .await
        .map(|_| ())
        .map_err(|error| match error {
            rcon_service::RconServiceError::NotEnabled
            | rcon_service::RconServiceError::NotRunning => {
                BackupError::RconRequiredForHotBackup
            }
            other => BackupError::Rcon(other.to_string()),
        })
}

pub async fn delete_backup(
    state: &Arc<AppState>,
    instance_id: &str,
    backup_id: &str,
) -> Result<(), BackupError> {
    let row: Option<(bool,)> = sqlx::query_as(
        "SELECT locked FROM backups WHERE id = ? AND instance_id = ?",
    )
    .bind(backup_id)
    .bind(instance_id)
    .fetch_optional(&state.pool)
    .await?;
    let (locked,) = row.ok_or(BackupError::NotFound)?;
    if locked {
        return Err(BackupError::Locked);
    }
    let zip = storage_dir(state, instance_id).join(format!("{backup_id}.zip"));
    tokio::fs::remove_file(&zip).await.ok();
    sqlx::query("DELETE FROM backups WHERE id = ?")
        .bind(backup_id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn delete_many_backups(
    state: &Arc<AppState>,
    instance_id: &str,
    ids: &[String],
) -> Result<usize, BackupError> {
    let mut deleted = 0usize;
    for id in ids {
        if delete_backup(state, instance_id, id).await.is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}

pub async fn lock_backup(
    state: &Arc<AppState>,
    instance_id: &str,
    backup_id: &str,
    locked: bool,
) -> Result<(), BackupError> {
    let rows = sqlx::query(
        "UPDATE backups SET locked = ? WHERE id = ? AND instance_id = ?",
    )
    .bind(locked)
    .bind(backup_id)
    .bind(instance_id)
    .execute(&state.pool)
    .await?;
    if rows.rows_affected() == 0 {
        return Err(BackupError::NotFound);
    }
    Ok(())
}

pub async fn restore_backup(
    state: &Arc<AppState>,
    instance_id: &str,
    backup_id: &str,
) -> Result<(), BackupError> {
    let iid: InstanceId =
        instance_id.parse().map_err(|_| BackupError::NotFound)?;
    if state.instances.contains_key(&iid) {
        return Err(BackupError::MustBeOffline);
    }
    let backup = get_backup(state, instance_id, backup_id).await?;
    let zip = storage_dir(state, instance_id).join(format!("{backup_id}.zip"));
    if !zip.exists() || backup.instance_id != instance_id {
        return Err(BackupError::NotFound);
    }
    let data_dir = data_dir_for(state, instance_id).await?;

    // Auto-backup the current state before overwriting.
    let _ = create_backup(
        state,
        instance_id,
        "manual",
        Some("pre-restore".to_string()),
    )
    .await;

    tokio::task::spawn_blocking(move || {
        restore_data_dir_atomically(&zip, &data_dir)
    })
    .await
    .map_err(|e| BackupError::Zip(e.to_string()))??;
    Ok(())
}

async fn get_backup(
    state: &Arc<AppState>,
    instance_id: &str,
    backup_id: &str,
) -> Result<BackupRecord, BackupError> {
    sqlx::query_as::<_, BackupRecord>(
		"SELECT id, instance_id, name, size_bytes, locked, trigger, hot, consistency, created_at FROM backups WHERE id = ? AND instance_id = ?",
	)
	.bind(backup_id)
	.bind(instance_id)
	.fetch_optional(&state.pool)
	.await?
	.ok_or(BackupError::NotFound)
}

fn restore_data_dir_atomically(
    zip: &Path,
    data_dir: &Path,
) -> Result<(), BackupError> {
    let parent = data_dir.parent().ok_or_else(|| {
        BackupError::Io(std::io::Error::other(
            "instance data dir has no parent",
        ))
    })?;
    std::fs::create_dir_all(parent).map_err(BackupError::Io)?;
    let nonce = Uuid::new_v4();
    let tmp = parent.join(format!(".restore-{nonce}"));
    let old = parent.join(format!(".restore-old-{nonce}"));
    std::fs::create_dir(&tmp).map_err(BackupError::Io)?;
    let result = (|| {
        unzip_to(zip, &tmp)?;
        if data_dir.exists() {
            std::fs::rename(data_dir, &old).map_err(BackupError::Io)?;
        }
        std::fs::rename(&tmp, data_dir).map_err(BackupError::Io)?;
        if old.exists() {
            std::fs::remove_dir_all(&old).map_err(BackupError::Io)?;
        }
        Ok(())
    })();
    if result.is_err() {
        if old.exists() && !data_dir.exists() {
            std::fs::rename(&old, data_dir).ok();
        }
        if tmp.exists() {
            std::fs::remove_dir_all(&tmp).ok();
        }
        if old.exists() {
            std::fs::remove_dir_all(&old).ok();
        }
    }
    result
}

pub async fn get_backup_schedule(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<BackupSchedule, BackupError> {
    let row: Option<(bool, String, i64)> = sqlx::query_as(
		"SELECT enabled, cron, retain_count FROM backup_schedules WHERE instance_id = ?",
	)
	.bind(instance_id)
	.fetch_optional(&state.pool)
	.await?;
    Ok(row
        .map(|(e, c, r)| BackupSchedule {
            enabled: e,
            cron: c,
            retain_count: r,
        })
        .unwrap_or(BackupSchedule {
            enabled: false,
            cron: "0 4 * * *".to_string(),
            retain_count: 5,
        }))
}

pub async fn set_backup_schedule(
    state: &Arc<AppState>,
    instance_id: &str,
    enabled: bool,
    cron: &str,
    retain_count: i64,
) -> Result<(), BackupError> {
    let _ = data_dir_for(state, instance_id).await?;
    if retain_count <= 0 {
        return Err(BackupError::InvalidSchedule(
            "retain_count must be greater than 0".into(),
        ));
    }
    validate_cron(cron).map_err(|_| {
        BackupError::InvalidSchedule("invalid cron expression".into())
    })?;
    let task_id = backup_task_id(instance_id);
    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::to_string(&BackupTaskPayload { retain_count })
        .map_err(|error| BackupError::InvalidSchedule(error.to_string()))?;
    sqlx::query(
		"INSERT INTO backup_schedules (instance_id, enabled, cron, retain_count) VALUES (?, ?, ?, ?) ON CONFLICT(instance_id) DO UPDATE SET enabled=excluded.enabled, cron=excluded.cron, retain_count=excluded.retain_count",
	)
	.bind(instance_id)
	.bind(enabled)
	.bind(cron)
	.bind(retain_count)
	.execute(&state.pool)
	.await?;
    sqlx::query(
		"INSERT INTO scheduled_tasks (id, instance_id, task_type, cron, enabled, payload, created_at, updated_at) VALUES (?, ?, 'backup', ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET cron = excluded.cron, enabled = excluded.enabled, payload = excluded.payload, updated_at = excluded.updated_at",
	)
	.bind(&task_id)
	.bind(instance_id)
	.bind(cron)
	.bind(enabled as i32)
	.bind(payload)
	.bind(&now)
	.bind(&now)
	.execute(&state.pool)
	.await?;
    Ok(())
}

fn backup_task_id(instance_id: &str) -> String {
    format!("backup-schedule-{instance_id}")
}

pub async fn enforce_backup_retention(
    state: &Arc<AppState>,
    instance_id: &str,
    retain_count: i64,
) -> Result<(), BackupError> {
    let total: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM backups WHERE instance_id = ? AND trigger = 'scheduled' AND locked = 0",
	)
	.bind(instance_id)
	.fetch_one(&state.pool)
	.await?;

    let excess = (total - retain_count).max(0);
    if excess == 0 {
        return Ok(());
    }

    let ids: Vec<(String,)> = sqlx::query_as(
		"SELECT id FROM backups WHERE instance_id = ? AND trigger = 'scheduled' AND locked = 0 ORDER BY created_at ASC LIMIT ?",
	)
	.bind(instance_id)
	.bind(excess)
	.fetch_all(&state.pool)
	.await?;

    for (id,) in ids {
        let zip = storage_dir(state, instance_id).join(format!("{id}.zip"));
        tokio::fs::remove_file(&zip).await.ok();
        sqlx::query("DELETE FROM backups WHERE id = ?")
            .bind(&id)
            .execute(&state.pool)
            .await?;
    }
    Ok(())
}

pub async fn rename_backup(
    state: &Arc<AppState>,
    instance_id: &str,
    backup_id: &str,
    name: String,
) -> Result<(), BackupError> {
    let rows = sqlx::query(
        "UPDATE backups SET name = ? WHERE id = ? AND instance_id = ?",
    )
    .bind(&name)
    .bind(backup_id)
    .bind(instance_id)
    .execute(&state.pool)
    .await?;
    if rows.rows_affected() == 0 {
        return Err(BackupError::NotFound);
    }
    Ok(())
}
