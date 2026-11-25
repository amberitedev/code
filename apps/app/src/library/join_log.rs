//! When each instance last joined each server, from the engine's `ServerJoined` events, for the
//! worlds lists.

use std::collections::HashMap;

use chrono::{DateTime, TimeZone, Utc};
use engine::worlds::ServerJoins;
use sqlx::SqlitePool;

pub async fn record(
    db: &SqlitePool,
    instance_id: &str,
    host: &str,
    port: u16,
    join_time: DateTime<Utc>,
) -> engine::Result<()> {
    let join_time = join_time.timestamp();
    let port = i64::from(port);
    sqlx::query!(
        "
		INSERT INTO join_log (instance_id, host, port, join_time)
		VALUES (?, ?, ?, ?)
		ON CONFLICT (instance_id, host, port) DO UPDATE SET
			join_time = excluded.join_time
		",
        instance_id,
        host,
        port,
        join_time,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn joins(
    db: &SqlitePool,
    instance_id: &str,
) -> engine::Result<ServerJoins> {
    let rows = sqlx::query!(
        "SELECT host, port, join_time FROM join_log WHERE instance_id = ?",
        instance_id,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ((row.host, row.port as u16), time(row.join_time)))
        .collect())
}

/// Every instance's joins, by instance id.
pub async fn all_joins(
    db: &SqlitePool,
) -> engine::Result<HashMap<String, ServerJoins>> {
    let rows =
        sqlx::query!("SELECT instance_id, host, port, join_time FROM join_log")
            .fetch_all(db)
            .await?;
    let mut joins = HashMap::<String, ServerJoins>::new();
    for row in rows {
        joins
            .entry(row.instance_id)
            .or_default()
            .insert((row.host, row.port as u16), time(row.join_time));
    }
    Ok(joins)
}

fn time(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .unwrap_or_else(Utc::now)
}
