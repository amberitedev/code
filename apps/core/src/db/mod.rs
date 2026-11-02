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

/// Copal's migrations, then the tables of the content store clone, which bedrock owns
/// (`bedrock::upstream::store::SCHEMA`), once, and the share history table, which
/// sharing-client owns (`sharing_client::history::SCHEMA`).
async fn migrate(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    let mut tx = pool.begin().await?;
    let has_store: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'store_blobs')",
    )
    .fetch_one(&mut *tx)
    .await?;
    if !has_store {
        sqlx::raw_sql(bedrock::upstream::store::SCHEMA)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::raw_sql(sharing_client::history::SCHEMA)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
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
                "app_metadata",
                "backend_owed",
                "backup_operations",
                "backup_schedules",
                "backups",
                "core",
                "instance_files",
                "jvm_flags",
                "pairing",
                "pending_packs",
                "port_mappings",
                "restarts",
                "runs",
                "server_content_entries",
                "server_content_sets",
                "server_files",
                "server_links",
                "servers",
                "sessions",
                "share_history",
                "store_blobs",
                "store_instance_files",
                "store_operations",
                "store_retained_refs",
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
