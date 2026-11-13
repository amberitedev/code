//! app.db: the App's own tables, in its settings folder. Rows about an instance keep its id
//! without a foreign key, since instances live in the engine's database.

use crate::settings::rows::SettingsRow;
use engine::{Engine, data::Settings};
use sqlx::SqlitePool;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use std::path::Path;
use std::time::Duration;

const DB_FILE: &str = "app.db";

/// The App's database of the engine it started.
pub fn pool(engine: &Engine) -> &SqlitePool {
    &crate::hooks::desktop(engine).db
}

/// Opens and migrates `settings_dir`'s app.db, backing it up first when this is a new version.
pub async fn open(settings_dir: &Path) -> engine::Result<SqlitePool> {
    let path = settings_dir.join(DB_FILE);
    if let Err(err) = engine::prelude::maybe_backup_existing_app_db(&path).await
    {
        tracing::warn!(
            "Continuing without a pre-migration app database backup: {err}"
        );
    }
    tokio::fs::create_dir_all(settings_dir).await?;
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .idle_timeout(Duration::from_secs(120))
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .busy_timeout(Duration::from_secs(30))
                .journal_mode(SqliteJournalMode::Wal)
                .optimize_on_close(true, None)
                .create_if_missing(true),
        )
        .await?;
    sqlx::migrate!().run(&pool).await?;
    if let Err(err) = remove_orphaned_skins(&pool).await {
        tracing::warn!("Failed to clean up skins of removed accounts: {err}");
    }
    Settings::migrate(&pool).await?;
    record_current_app_version(&pool).await?;
    Ok(pool)
}

async fn record_current_app_version(pool: &SqlitePool) -> engine::Result<()> {
    let previous_version = sqlx::query_scalar!(
        "SELECT value FROM app_metadata WHERE key = 'app_version'"
    )
    .fetch_optional(pool)
    .await?;
    let already_used_sync_update = previous_version
        .as_deref()
        .and_then(|version| {
            let mut parts = version.split('.');
            Some((
                parts.next()?.parse::<u64>().ok()?,
                parts.next()?.parse::<u64>().ok()?,
            ))
        })
        .is_some_and(|version| version >= (0, 20));

    if env!("CARGO_PKG_VERSION").starts_with("0.20.")
        && already_used_sync_update
    {
        let mut settings = Settings::get(pool).await?;
        if settings.pending_update_toast_for_version.is_some() {
            settings.pending_update_toast_for_version = None;
            settings.update(pool).await?;
        }
    }

    sqlx::query!(
        "
		INSERT INTO app_metadata (key, value, updated_at)
		VALUES ('app_version', ?, unixepoch())
		ON CONFLICT(key) DO UPDATE SET
			value = excluded.value,
			updated_at = excluded.updated_at
		",
        env!("CARGO_PKG_VERSION"),
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn remove_orphaned_skins(pool: &SqlitePool) -> engine::Result<()> {
    sqlx::query!(
        "DELETE FROM custom_minecraft_skins WHERE minecraft_user_uuid NOT IN (SELECT uuid FROM minecraft_users)"
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes the App's rows about a removed instance.
pub async fn forget_instance(
    pool: &SqlitePool,
    instance_id: &str,
) -> engine::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "DELETE FROM hidden_instances WHERE instance_id = ?",
        instance_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM instance_game_option_pack_bases WHERE instance_id = ?",
        instance_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM instance_game_option_update_state WHERE instance_id = ?",
        instance_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM instance_server_pack_state WHERE instance_id = ?",
        instance_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM instance_server_projection_entries WHERE instance_id = ?",
        instance_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM instance_servers WHERE instance_id = ?",
        instance_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM instance_sync_checkpoints WHERE instance_id = ?",
        instance_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM instance_sync_preferences WHERE instance_id = ?",
        instance_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM screenshots WHERE instance_id = ?", instance_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
