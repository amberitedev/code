//! History entries as rows of `share_history` ([`super::SCHEMA`]), one JSON entry per version.

use std::collections::HashMap;

use sqlx::SqlitePool;
use sqlx::types::Json;

use super::HistoryEntry;

/// Saves `entry` for `share_id`, replacing one with the same version.
pub(super) async fn save_entry(
    db: &SqlitePool,
    share_id: &str,
    entry: &HistoryEntry,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO share_history (share_id, version, entry) VALUES (?, ?, ?) \
         ON CONFLICT (share_id, version) DO UPDATE SET entry = excluded.entry",
    )
    .bind(share_id)
    .bind(entry.version)
    .bind(Json(entry))
    .execute(db)
    .await?;
    Ok(())
}

/// Every entry of `share_id`, in any order.
pub(super) async fn entries(
    db: &SqlitePool,
    share_id: &str,
) -> sqlx::Result<Vec<HistoryEntry>> {
    let rows: Vec<Json<HistoryEntry>> = sqlx::query_scalar(
        "SELECT entry FROM share_history WHERE share_id = ?",
    )
    .bind(share_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|Json(entry)| entry).collect())
}

pub(super) async fn delete_entry(
    db: &SqlitePool,
    share_id: &str,
    version: i32,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM share_history WHERE share_id = ? AND version = ?")
        .bind(share_id)
        .bind(version)
        .execute(db)
        .await?;
    Ok(())
}

/// How many entries each share with any has.
pub async fn counts(db: &SqlitePool) -> sqlx::Result<HashMap<String, usize>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT share_id, COUNT(*) FROM share_history GROUP BY share_id",
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(share_id, entries)| (share_id, entries as usize))
        .collect())
}
