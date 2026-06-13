use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{application::state::AppState, domain::instance::InstanceId};

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("instance not found")]
    NotFound,
    #[error("invalid task type")]
    InvalidType,
    #[error("invalid cron expression")]
    InvalidCron,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Backup,
    Restart,
    Command,
    Announce,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Backup => "backup",
            TaskType::Restart => "restart",
            TaskType::Command => "command",
            TaskType::Announce => "announce",
        }
    }
}

impl std::str::FromStr for TaskType {
    type Err = TaskError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "backup" => Ok(TaskType::Backup),
            "restart" => Ok(TaskType::Restart),
            "command" => Ok(TaskType::Command),
            "announce" => Ok(TaskType::Announce),
            _ => Err(TaskError::InvalidType),
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ScheduledTask {
    pub id: String,
    pub instance_id: String,
    pub task_type: String,
    pub cron: String,
    pub enabled: i32,
    pub payload: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_run_at: Option<String>,
}

impl ScheduledTask {
    pub fn is_enabled(&self) -> bool {
        self.enabled != 0
    }
}

impl ScheduledTask {
    pub fn payload_json(&self) -> Option<Value> {
        self.payload
            .as_ref()
            .and_then(|p| serde_json::from_str(p).ok())
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskBody {
    pub task_type: String,
    pub cron: String,
    pub payload: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskBody {
    pub cron: Option<String>,
    pub enabled: Option<bool>,
    pub payload: Option<Value>,
}

fn validate_cron(cron: &str) -> Result<(), TaskError> {
    let parts: Vec<&str> = cron.split_whitespace().collect();
    if parts.len() != 5 {
        return Err(TaskError::InvalidCron);
    }
    Ok(())
}

pub async fn list_tasks(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<Vec<ScheduledTask>, TaskError> {
    let _ = instance_id
        .parse::<InstanceId>()
        .map_err(|_| TaskError::NotFound)?;
    let rows: Vec<ScheduledTask> = sqlx::query_as::<_, ScheduledTask>(
		"SELECT id, instance_id, task_type, cron, enabled, payload, created_at, updated_at, last_run_at
		FROM scheduled_tasks WHERE instance_id = ? ORDER BY created_at DESC",
	)
	.bind(instance_id)
	.fetch_all(&state.pool)
	.await?;
    Ok(rows)
}

pub async fn create_task(
    state: &Arc<AppState>,
    instance_id: &str,
    body: CreateTaskBody,
) -> Result<ScheduledTask, TaskError> {
    let _ = instance_id
        .parse::<InstanceId>()
        .map_err(|_| TaskError::NotFound)?;
    let task_type = body.task_type.parse::<TaskType>()?;
    validate_cron(&body.cron)?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let payload_str = body.payload.map(|p| p.to_string());

    sqlx::query(
		"INSERT INTO scheduled_tasks (id, instance_id, task_type, cron, enabled, payload, created_at, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
	)
	.bind(&id)
	.bind(instance_id)
	.bind(task_type.as_str())
	.bind(&body.cron)
	.bind(1i32)
	.bind(&payload_str)
	.bind(&now)
	.bind(&now)
	.execute(&state.pool)
	.await?;

    Ok(ScheduledTask {
        id,
        instance_id: instance_id.to_string(),
        task_type: task_type.as_str().to_string(),
        cron: body.cron,
        enabled: 1i32,
        payload: payload_str,
        created_at: now.clone(),
        updated_at: now,
        last_run_at: None,
    })
}

pub async fn get_task(
    state: &Arc<AppState>,
    instance_id: &str,
    task_id: &str,
) -> Result<ScheduledTask, TaskError> {
    let row: Option<ScheduledTask> = sqlx::query_as::<_, ScheduledTask>(
		"SELECT id, instance_id, task_type, cron, enabled, payload, created_at, updated_at, last_run_at
		FROM scheduled_tasks WHERE id = ? AND instance_id = ?",
	)
	.bind(task_id)
	.bind(instance_id)
	.fetch_optional(&state.pool)
	.await?;

    row.ok_or(TaskError::NotFound)
}

pub async fn update_task(
    state: &Arc<AppState>,
    instance_id: &str,
    task_id: &str,
    body: UpdateTaskBody,
) -> Result<ScheduledTask, TaskError> {
    let existing = get_task(state, instance_id, task_id).await?;
    let now = chrono::Utc::now().to_rfc3339();

    let has_cron = body.cron.is_some();
    let cron = body.cron.unwrap_or_else(|| existing.cron.clone());
    if has_cron {
        validate_cron(&cron)?;
    }
    let enabled = body.enabled.unwrap_or(existing.is_enabled());
    let payload = body.payload.map(|p| p.to_string()).or(existing.payload);

    sqlx::query(
		"UPDATE scheduled_tasks SET cron = ?, enabled = ?, payload = ?, updated_at = ? WHERE id = ?",
	)
	.bind(&cron)
	.bind(enabled as i32)
	.bind(&payload)
	.bind(&now)
	.bind(task_id)
	.execute(&state.pool)
	.await?;

    Ok(ScheduledTask {
        id: task_id.to_string(),
        instance_id: instance_id.to_string(),
        task_type: existing.task_type,
        cron,
        enabled: if enabled { 1i32 } else { 0i32 },
        payload,
        created_at: existing.created_at,
        updated_at: now,
        last_run_at: existing.last_run_at,
    })
}

pub async fn delete_task(
    state: &Arc<AppState>,
    instance_id: &str,
    task_id: &str,
) -> Result<(), TaskError> {
    let result = sqlx::query(
        "DELETE FROM scheduled_tasks WHERE id = ? AND instance_id = ?",
    )
    .bind(task_id)
    .bind(instance_id)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(TaskError::NotFound);
    }
    Ok(())
}

pub async fn record_task_run(
    state: &Arc<AppState>,
    task_id: &str,
) -> Result<(), TaskError> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE scheduled_tasks SET last_run_at = ? WHERE id = ?")
        .bind(&now)
        .bind(task_id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn list_all_enabled_tasks(
    state: &Arc<AppState>,
) -> Result<Vec<ScheduledTask>, TaskError> {
    let rows: Vec<ScheduledTask> = sqlx::query_as::<_, ScheduledTask>(
		"SELECT id, instance_id, task_type, cron, enabled, payload, created_at, updated_at, last_run_at
		FROM scheduled_tasks WHERE enabled = 1 ORDER BY instance_id, created_at DESC",
	)
	.fetch_all(&state.pool)
	.await?;
    Ok(rows)
}
