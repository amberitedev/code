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
    let migrator = sqlx::migrate!("./migrations");
    adopt_edited_001(pool, &migrator.migrations[0].checksum).await?;
    migrator.run(pool).await?;
    Ok(())
}

/// The SHA-384 sqlx recorded for `001_init.sql` as PR 44 had edited it (`f2dabebef`), with the
/// backup tables inside. Installs from then applied it.
const EDITED_001: &str = "6f89a3cf800541a036235342d455d69dec169189e656af5668591ea88aa73f465a1343c3e95270048ce37b6d7fe5e980";

/// A database that applied 001 as PR 44 had edited it records that version's checksum, which
/// sqlx then refuses for the restored 001. Give it the restored one's: the schemas match but
/// for the backup tables, which 002 creates only where they are missing.
async fn adopt_edited_001(
    pool: &SqlitePool,
    restored: &[u8],
) -> sqlx::Result<()> {
    let migrated: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    if migrated == 0 {
        return Ok(());
    }
    let edited = hex::decode(EDITED_001).expect("a hex constant");
    sqlx::query(
        "UPDATE _sqlx_migrations SET checksum = ? WHERE version = 1 AND checksum = ?",
    )
    .bind(restored)
    .bind(edited)
    .execute(pool)
    .await?;
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
                "backup_operations",
                "backup_schedules",
                "backups",
                "content_files",
                "core",
                "jvm_flags",
                "packs",
                "pairing",
                "port_mappings",
                "restarts",
                "runs",
                "servers",
                "sessions",
                "used_tickets",
                "users",
            ]
        );
    }

    #[tokio::test]
    async fn a_database_from_before_the_backup_engine_is_brought_up_to_date() {
        let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().in_memory(true))
            .await
            .unwrap();
        // As an install from before step 5 left it: 001 alone, unchanged since it shipped.
        let all = sqlx::migrate!("./migrations");
        let mut first = sqlx::migrate::Migrator::new(std::path::Path::new(
            concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"),
        ))
        .await
        .unwrap();
        first.migrations = all.migrations[..1].to_vec().into();
        first.run(&pool).await.unwrap();
        migrate(&pool).await.unwrap();
        let applied: Vec<i64> = sqlx::query_scalar(
            "SELECT version FROM _sqlx_migrations ORDER BY version",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(applied, [1, 2, 3, 4]);
        sqlx::query("SELECT COUNT(*) FROM backup_operations, backup_schedules")
            .execute(&pool)
            .await
            .unwrap();
    }

    /// 001 as PR 44 had edited it, as Linux builds embedded it.
    fn edited_001() -> String {
        include_str!("fixtures/001_from_pr44.sql").replace(
            "
", "
",
        )
    }

    #[test]
    fn the_edited_001_checksum_is_that_file_s() {
        use sha2::Digest;
        assert_eq!(
            hex::encode(sha2::Sha384::digest(edited_001().as_bytes())),
            EDITED_001
        );
    }

    #[tokio::test]
    async fn a_database_made_by_the_edited_001_starts() {
        let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().in_memory(true))
            .await
            .unwrap();
        // As an install from PR 44 until now left it: the edited 001, recorded with its checksum.
        let mut edited = sqlx::migrate::Migrator::new(std::path::Path::new(
            concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"),
        ))
        .await
        .unwrap();
        edited.migrations = vec![sqlx::migrate::Migration::new(
            1,
            "init".into(),
            sqlx::migrate::MigrationType::Simple,
            edited_001().into(),
            false,
        )]
        .into();
        edited.run(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO backup_schedules (server_id, interval_hours, keep)              SELECT id, 6, 3 FROM servers",
        )
        .execute(&pool)
        .await
        .unwrap();

        migrate(&pool).await.unwrap();
        let applied: Vec<i64> = sqlx::query_scalar(
            "SELECT version FROM _sqlx_migrations ORDER BY version",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(applied, [1, 2, 3, 4]);
        // And again, as every later start does.
        migrate(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn core_id_is_made_once() {
        let pool = memory().await;
        let first = core_id(&pool).await.unwrap();
        assert_eq!(core_id(&pool).await.unwrap(), first);
    }
}
