use std::sync::Arc;

use chrono::{Datelike, Timelike, Utc};
use serde_json::Value;

use crate::application::{
    backup_service::create_backup,
    instance_status_service::{restart_instance, send_command},
    state::AppState,
    task_service::{list_all_enabled_tasks, record_task_run, TaskType},
};

/// Background task: wakes every 60 seconds and fires scheduled instance tasks when due.
pub async fn run_task_scheduler(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        if let Err(e) = tick(&state).await {
            tracing::error!("task scheduler error: {e}");
        }
    }
}

async fn tick(state: &Arc<AppState>) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let tasks = match list_all_enabled_tasks(state).await {
        Ok(t) => t.into_iter().filter(|t| t.is_enabled()).collect::<Vec<_>>(),
        Err(e) => {
            tracing::error!("failed to list tasks: {e}");
            return Ok(());
        }
    };

    for task in tasks {
        if !cron_matches(
            &task.cron,
            now.minute(),
            now.hour(),
            now.day(),
            now.month(),
            now.weekday().num_days_from_sunday(),
        ) {
            continue;
        }

        tracing::info!(
            "firing scheduled task {} for instance {} (type: {})",
            task.id,
            task.instance_id,
            task.task_type
        );

        if let Err(e) = execute_task(state, &task.instance_id, &task.task_type, task.payload_json()).await {
            tracing::error!(
                "scheduled task {} for instance {} failed: {e}",
                task.id,
                task.instance_id
            );
        }

        if let Err(e) = record_task_run(state, &task.id).await {
            tracing::warn!("failed to record task run for {}: {e}", task.id);
        }
    }

    Ok(())
}

async fn execute_task(
    state: &Arc<AppState>,
    instance_id: &str,
    task_type: &str,
    payload: Option<Value>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let iid = instance_id.parse::<crate::domain::instance::InstanceId>()?;
    match task_type.parse::<TaskType>()? {
        TaskType::Backup => {
            create_backup(state, instance_id, "automated", None).await?;
        }
        TaskType::Restart => {
            restart_instance(state, &iid).await?;
        }
        TaskType::Command => {
            let command = payload
                .as_ref()
                .and_then(|p| p.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("say Scheduled task executed")
                .to_string();
            send_command(state, &iid, command).await?;
        }
        TaskType::Announce => {
            let message = payload
                .as_ref()
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("[Server] Scheduled announcement");
            send_command(state, &iid, format!("say {message}")).await?;
        }
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
