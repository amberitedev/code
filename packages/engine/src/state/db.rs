use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use sqlx::{Pool, Sqlite};
use std::path::Path;
use std::time::Duration;

/// The engine's database, in its data directory.
pub const DB_FILE: &str = "engine.db";

/// Every table of engine.db. A host keeps its own tables in its own database.
pub const TABLES: &[&str] = &[
    "_sqlx_migrations",
    "app_metadata",
    "attached_world_data",
    "cache",
    "install_job_requests",
    "install_jobs",
    "instance_content_entries",
    "instance_content_locks",
    "instance_content_set_remote_refs",
    "instance_content_set_sync_state",
    "instance_content_sets",
    "instance_content_update_checks",
    "instance_files",
    "instance_group_memberships",
    "instance_groups",
    "instance_icon_configs",
    "instance_launch_overrides",
    "instance_links",
    "instance_quarantines",
    "instances",
    "java_versions",
    "join_log",
    "processes",
    "share_history",
    "shared_instance_applied_files",
    "store_blobs",
    "store_instance_files",
    "store_operations",
    "store_retained_refs",
];

/// Opens and migrates `data_dir`'s engine.db.
pub(crate) async fn open(data_dir: &Path) -> crate::Result<Pool<Sqlite>> {
    crate::util::io::create_dir_all(data_dir).await?;
    let conn_options = SqliteConnectOptions::new()
        .filename(data_dir.join(DB_FILE))
        .busy_timeout(Duration::from_secs(30))
        .journal_mode(SqliteJournalMode::Wal)
        .optimize_on_close(true, None)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .idle_timeout(Duration::from_secs(120))
        .connect_with(conn_options)
        .await?;

    sqlx::migrate!().run(&pool).await?;
    // The share history table is sharing-client's, the same as Core's.
    sqlx::raw_sql(sharing_client::history::SCHEMA)
        .execute(&pool)
        .await?;

    Ok(pool)
}

/// Replaces `data_dir`'s engine.db with a copy of `pool`'s.
pub(crate) async fn copy(
    pool: &Pool<Sqlite>,
    data_dir: &Path,
) -> crate::Result<()> {
    let copy = data_dir.join(format!("{DB_FILE}.{}.tmp", uuid::Uuid::new_v4()));
    super::db_backup::create_sqlite_snapshot(
        &mut *pool.acquire().await?,
        &copy,
    )
    .await?;
    remove(data_dir).await?;
    crate::util::io::rename_or_move(&copy, &data_dir.join(DB_FILE)).await?;
    Ok(())
}

/// Deletes `data_dir`'s engine.db, which must be closed.
pub(crate) async fn remove(data_dir: &Path) -> crate::Result<()> {
    for file in [DB_FILE, "engine.db-wal", "engine.db-shm"] {
        match tokio::fs::remove_file(data_dir.join(file)).await {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(error.into());
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn engine_db_holds_only_engine_tables() {
        let dir = tempfile::tempdir().unwrap();
        let pool = super::open(dir.path()).await.unwrap();
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(tables, super::TABLES);
    }
}
