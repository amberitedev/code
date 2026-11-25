//! SQLite snapshots: engine.db's copies and the support bundle use them, and so do the App's
//! database backups.

use crate::ErrorKind;
use sqlx::sqlite::SqliteConnection;
use std::path::{Path, PathBuf};

/// Copies the database file and its WAL byte-for-byte. Used when `VACUUM INTO`
/// fails, which usually means the database is corrupt and the raw files are
/// what's needed to recover it.
pub async fn copy_db_files(
    db_path: &Path,
    backup_path: &Path,
) -> crate::Result<()> {
    crate::util::io::copy(db_path, backup_path).await?;

    let db_wal_path = wal_path(db_path);
    if db_wal_path.try_exists()? {
        crate::util::io::copy(&db_wal_path, wal_path(backup_path)).await?;
    }

    Ok(())
}

pub async fn remove_partial_backup(backup_path: &Path) {
    for path in [backup_path.to_path_buf(), wal_path(backup_path)] {
        if let Err(err) = crate::util::io::remove_file(&path).await
            && err.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(
                "Failed to remove partial app database backup at {}: {err}",
                path.display()
            );
        }
    }
}

fn wal_path(db_path: &Path) -> PathBuf {
    let mut path = db_path.as_os_str().to_owned();
    path.push("-wal");
    PathBuf::from(path)
}

pub async fn create_sqlite_snapshot(
    conn: &mut SqliteConnection,
    backup_path: &Path,
) -> crate::Result<()> {
    let backup_path = backup_path
        .to_str()
        .ok_or(ErrorKind::UTFError(backup_path.to_path_buf()))?;

    sqlx::query!("VACUUM INTO ?", backup_path)
        .execute(&mut *conn)
        .await?;

    Ok(())
}
