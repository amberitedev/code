//! The SQLite database in `db/copal.db`. Code queries it directly with `sqlx`; there is no
//! repository layer. Tests use [`memory`].

use std::{path::Path, time::Duration};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool};

/// Open the database at `path`, creating it when missing, and apply migrations.
pub async fn open(path: &Path) -> sqlx::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePool::connect_with(options).await?;
    migrate(&pool).await?;
    Ok(pool)
}

/// A fresh migrated database in memory, for tests. One connection, because every in-memory
/// connection is its own database.
#[cfg(test)]
pub async fn memory() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true);
    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

async fn migrate(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

/// This Core's id, made on first start. It is the audience of every ticket the backend signs for
/// this Core.
pub async fn core_id(pool: &SqlitePool) -> sqlx::Result<String> {
    sqlx::query(
        "INSERT INTO core (id, core_id, created_at) VALUES (1, ?, unixepoch()) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(pool)
    .await?;
    sqlx::query_scalar("SELECT core_id FROM core WHERE id = 1")
        .fetch_one(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_apply_from_empty() {
        let pool = memory().await;
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            tables,
            [
                "access",
                "activity",
                "backend_owed",
                "backups",
                "content_files",
                "core",
                "packs",
                "pairing",
                "runs",
                "servers",
                "sessions",
                "used_tickets",
                "users",
            ]
        );
    }

    #[tokio::test]
    async fn core_id_is_made_once() {
        let pool = memory().await;
        let first = core_id(&pool).await.unwrap();
        assert_eq!(core_id(&pool).await.unwrap(), first);
    }
}
