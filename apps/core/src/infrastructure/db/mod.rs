pub mod instance_repo;
pub mod java_repo;
pub mod modpack_repo;

use std::{path::Path, time::Duration};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool};

/// Connect to (or create) the SQLite database at the given path.
/// Uses WAL journal mode and a 5-second busy timeout to reduce contention.
pub async fn connect(path: &Path) -> color_eyre::eyre::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePool::connect_with(options).await?;
    Ok(pool)
}
