use std::sync::Arc;

use chrono::{Datelike, Timelike, Utc};

use crate::application::{
    backup_service::{create_backup, storage_dir},
    state::AppState,
};

/// Background task: wakes every 60 seconds and fires automated backups when due.
pub async fn run_backup_scheduler(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        if let Err(e) = tick(&state).await {
            tracing::error!("backup scheduler error: {e}");
        }
    }
}

async fn tick(state: &Arc<AppState>) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let schedules: Vec<(String, String, i64)> = sqlx::query_as(
		"SELECT instance_id, cron, retain_count FROM backup_schedules WHERE enabled = 1",
	)
	.fetch_all(&state.pool)
	.await?;

    for (instance_id, cron, retain_count) in schedules {
        if !cron_matches(
            &cron,
            now.minute(),
            now.hour(),
            now.day(),
            now.month(),
            now.weekday().num_days_from_sunday(),
        ) {
            continue;
        }
        if let Err(e) =
            create_backup(state, &instance_id, "automated", None).await
        {
            tracing::error!("automated backup for {instance_id} failed: {e}");
            continue;
        }
        if let Err(e) = enforce_retain(state, &instance_id, retain_count).await
        {
            tracing::warn!("retain enforcement for {instance_id} failed: {e}");
        }
    }
    Ok(())
}

async fn enforce_retain(
    state: &Arc<AppState>,
    instance_id: &str,
    retain_count: i64,
) -> Result<(), sqlx::Error> {
    let total: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM backups WHERE instance_id = ? AND trigger = 'automated' AND locked = 0",
	)
	.bind(instance_id)
	.fetch_one(&state.pool)
	.await?;

    let excess = (total - retain_count).max(0);
    if excess == 0 {
        return Ok(());
    }

    let ids: Vec<(String,)> = sqlx::query_as(
		"SELECT id FROM backups WHERE instance_id = ? AND trigger = 'automated' AND locked = 0 ORDER BY created_at ASC LIMIT ?",
	)
	.bind(instance_id)
	.bind(excess)
	.fetch_all(&state.pool)
	.await?;

    for (id,) in ids {
        let zip = storage_dir(state, instance_id).join(format!("{id}.zip"));
        tokio::fs::remove_file(&zip).await.ok();
        sqlx::query("DELETE FROM backups WHERE id = ?")
            .bind(&id)
            .execute(&state.pool)
            .await?;
    }
    Ok(())
}

fn cron_matches(
    cron: &str,
    minute: u32,
    hour: u32,
    dom: u32,
    month: u32,
    dow: u32,
) -> bool {
    let parts: Vec<&str> = cron.split_whitespace().collect();
    if parts.len() != 5 {
        return false;
    }
    field_matches(parts[0], minute)
        && field_matches(parts[1], hour)
        && field_matches(parts[2], dom)
        && field_matches(parts[3], month)
        && field_matches(parts[4], dow)
}

fn field_matches(field: &str, value: u32) -> bool {
    if field == "*" {
        return true;
    }
    field.parse::<u32>().map(|n| n == value).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{cron_matches, field_matches};

    #[test]
    fn wildcard_matches_any() {
        assert!(field_matches("*", 0));
        assert!(field_matches("*", 59));
    }

    #[test]
    fn exact_matches_only_itself() {
        assert!(field_matches("4", 4));
        assert!(!field_matches("4", 5));
    }

    #[test]
    fn daily_4am_fires_at_correct_time() {
        // "0 4 * * *" — fire at 04:00 every day
        assert!(cron_matches("0 4 * * *", 0, 4, 15, 5, 3));
        assert!(!cron_matches("0 4 * * *", 1, 4, 15, 5, 3));
        assert!(!cron_matches("0 4 * * *", 0, 5, 15, 5, 3));
    }
}
