//! The icon editor's recently used icons.

use chrono::Utc;
use engine::data::IconConfig;
use sqlx::SqlitePool;

const KEPT: i64 = 16;

pub async fn list(pool: &SqlitePool) -> engine::Result<Vec<IconConfig>> {
    sqlx::query!(
        "
		SELECT background, symbol
		FROM recent_instance_icon_configs
		ORDER BY used_at DESC, background, symbol
		LIMIT ?
		",
        KEPT,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| {
        Ok(IconConfig {
            background: serde_json::from_str(&row.background)?,
            symbol: row.symbol,
        })
    })
    .collect()
}

pub async fn record(
    config: &IconConfig,
    pool: &SqlitePool,
) -> engine::Result<()> {
    let background = serde_json::to_string(&config.background)?;
    let used_at = Utc::now().timestamp_millis();
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query!(
        "
		INSERT INTO recent_instance_icon_configs (background, symbol, used_at)
		VALUES (?, ?, ?)
		ON CONFLICT (background, symbol) DO UPDATE SET
			used_at = excluded.used_at
		",
        background,
        config.symbol,
        used_at,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "
		DELETE FROM recent_instance_icon_configs
		WHERE rowid NOT IN (
			SELECT rowid
			FROM recent_instance_icon_configs
			ORDER BY used_at DESC, background, symbol
			LIMIT ?
		)
		",
        KEPT,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
