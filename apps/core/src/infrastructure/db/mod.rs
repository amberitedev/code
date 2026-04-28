pub mod instance_repo;
pub mod modpack_repo;

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

/// Connect to (or create) the SQLite database at the given path.
pub async fn connect(path: &Path) -> color_eyre::eyre::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    Ok(pool)
}
