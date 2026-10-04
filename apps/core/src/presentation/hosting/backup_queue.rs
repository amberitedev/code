//! Durable Hosting operation history around the existing backup service.
//! Workers serialize operations for each server; progress is a real state, not a timer.
use super::{notify_changed, record, world, ApiResult};
use crate::{
    application::{backup_service, state::AppState},
    presentation::{error::ApiError, extractors::AuthUser},
};
use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use dashmap::DashMap;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

#[derive(Default)]
struct Queue {
    initialized: OnceCell<()>,
    servers: DashMap<String, Arc<Mutex<()>>>,
}

#[derive(Clone, sqlx::FromRow)]
struct Operation {
    id: i64,
    instance_id: String,
    backup_id: String,
    name: String,
    operation_type: String,
    state: String,
    scheduled_for: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    error: Option<String>,
    acknowledged: bool,
    user_id: String,
    safety_name: Option<String>,
}

#[derive(Deserialize)]
struct Name {
    name: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/servers/:id/worlds/:wid/backups-queue", get(list).post(create))
        .route("/v1/servers/:id/worlds/:wid/backups-queue/delete-many", post(delete_many))
        .route("/v1/servers/:id/worlds/:wid/backups-queue/:bid", delete(remove))
        .route("/v1/servers/:id/worlds/:wid/backups-queue/:bid/restore", post(restore))
        .route("/v1/servers/:id/worlds/:wid/backups-queue/:bid/retry", post(retry))
        .route("/v1/servers/:id/worlds/:wid/backups-queue/history/:kind/:operation/:action", post(history_action))
        .layer(Extension(Arc::new(Queue::default())))
}

impl Queue {
    async fn init(&self, state: &AppState) -> Result<(), ApiError> {
        self.initialized.get_or_try_init(|| async {
            sqlx::query("CREATE TABLE IF NOT EXISTS hosting_backup_operations (id INTEGER PRIMARY KEY AUTOINCREMENT, instance_id TEXT NOT NULL, backup_id TEXT NOT NULL, name TEXT NOT NULL, operation_type TEXT NOT NULL, state TEXT NOT NULL, scheduled_for TEXT NOT NULL, started_at TEXT, completed_at TEXT, error TEXT, acknowledged INTEGER NOT NULL DEFAULT 0, user_id TEXT NOT NULL, safety_name TEXT)")
                .execute(&state.pool).await?;
            sqlx::query("CREATE INDEX IF NOT EXISTS hosting_backup_operations_instance ON hosting_backup_operations(instance_id, id)").execute(&state.pool).await?;
            // The prior process cannot still own work in this isolated database.
            // A completed archive can survive a crash before its history commit.
            sqlx::query("UPDATE hosting_backup_operations SET state = CASE WHEN operation_type = 'create' AND EXISTS (SELECT 1 FROM backups WHERE backups.id = hosting_backup_operations.backup_id) THEN 'completed' ELSE 'failed' END, error = CASE WHEN operation_type = 'create' AND EXISTS (SELECT 1 FROM backups WHERE backups.id = hosting_backup_operations.backup_id) THEN NULL ELSE 'Core restarted before this operation completed' END, completed_at = ? WHERE state IN ('pending','ongoing')")
                .bind(chrono::Utc::now().to_rfc3339()).execute(&state.pool).await?;
            Ok::<(), sqlx::Error>(())
        }).await.map_err(db_error)?;
        Ok(())
    }
}

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::Internal(error.to_string())
}

async fn authorize(
    user: &AuthUser,
    state: &Arc<AppState>,
    queue: &Queue,
    id: String,
    wid: String,
) -> Result<String, ApiError> {
    let id = world(id, wid)?;
    let r = record(state, user, &id, "server:backups").await?;
    queue.init(state).await?;
    Ok(r.id.to_string())
}

async fn operations(
    state: &AppState,
    id: &str,
) -> Result<Vec<Operation>, ApiError> {
    sqlx::query_as::<_,Operation>("SELECT * FROM hosting_backup_operations WHERE instance_id = ? ORDER BY id DESC")
        .bind(id).fetch_all(&state.pool).await.map_err(db_error)
}

fn operation_json(op: &Operation) -> Value {
    json!({"operation_id": op.id, "operation_type": op.operation_type, "state": op.state,
        "scheduled_for": op.scheduled_for, "started_at": op.started_at, "completed_at": op.completed_at,
        "has_parent": false, "error": op.error, "should_prompt": !op.acknowledged,
        "synthetic_legacy": false, "user_info": {"id": op.user_id, "username": "Local developer", "avatar_url": null}})
}

async fn list(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Extension(queue): Extension<Arc<Queue>>,
    Path((id, wid)): Path<(String, String)>,
) -> ApiResult {
    let id = authorize(&user, &state, &queue, id, wid).await?;
    let ops = operations(&state, &id).await?;
    let records = backup_service::list_backups(&state, &id).await?;
    let mut backups: Vec<Value> = records
        .iter()
        .map(crate::presentation::hosting::models::backup)
        .collect();
    let mut seen_creates = std::collections::HashSet::new();
    for op in &ops {
        if op.operation_type == "create"
            && seen_creates.insert(op.backup_id.clone())
            && op.state != "completed"
            && !backups
                .iter()
                .any(|b| b["id"].as_str() == Some(&op.backup_id))
        {
            let status = match op.state.as_str() {
                "pending" => "pending",
                "ongoing" => "in_progress",
                _ => "error",
            };
            backups.push(json!({"id": op.backup_id, "name": op.name, "created_at": op.scheduled_for,
                "status": status, "locked": false, "automated": false, "history": []}));
        }
    }
    for backup in &mut backups {
        backup["history"] = json!(ops
            .iter()
            .filter(|op| Some(op.backup_id.as_str()) == backup["id"].as_str())
            .map(operation_json)
            .collect::<Vec<_>>());
    }
    let active: Vec<Value> = ops
        .iter()
        .filter(|op| matches!(op.state.as_str(), "pending" | "ongoing"))
        .map(|op| {
            let mut value = operation_json(op);
            value["backup_id"] = json!(op.backup_id);
            value
        })
        .collect();
    Ok(Json(
        json!({"backups": backups, "active_operations": active}),
    ))
}

fn name(value: String) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 200 {
        return Err(ApiError::BadRequest(
            "backup name must contain 1-200 characters".into(),
        ));
    }
    Ok(value.into())
}

async fn enqueue(
    state: Arc<AppState>,
    queue: Arc<Queue>,
    instance_id: String,
    backup_id: String,
    display_name: String,
    kind: &str,
    user_id: String,
    safety_name: Option<String>,
) -> Result<(), ApiError> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query("INSERT INTO hosting_backup_operations(instance_id,backup_id,name,operation_type,state,scheduled_for,user_id,safety_name) VALUES (?,?,?,?,'pending',?,?,?)")
        .bind(&instance_id).bind(&backup_id).bind(&display_name).bind(kind).bind(&now).bind(&user_id).bind(&safety_name)
        .execute(&state.pool).await.map_err(db_error)?;
    let op_id = result.last_insert_rowid();
    let lock = queue
        .servers
        .entry(instance_id.clone())
        .or_default()
        .clone();
    notify_changed(&state, &instance_id, false).await?;
    tokio::spawn(async move {
        let _guard = lock.lock().await;
        let files_changed = match run(&state, op_id).await {
            Ok(files_changed) => files_changed,
            Err(error) => {
                tracing::error!(
                    operation_id = op_id,
                    ?error,
                    "Hosting backup worker failed"
                );
                false
            }
        };
        if let Err(error) =
            notify_changed(&state, &instance_id, files_changed).await
        {
            tracing::warn!(?error, "Backup refresh notification failed");
        }
    });
    Ok(())
}

async fn run(state: &Arc<AppState>, op_id: i64) -> Result<bool, ApiError> {
    let claimed = sqlx::query("UPDATE hosting_backup_operations SET state='ongoing', started_at=? WHERE id=? AND state='pending'")
        .bind(chrono::Utc::now().to_rfc3339()).bind(op_id).execute(&state.pool).await.map_err(db_error)?;
    if claimed.rows_affected() == 0 {
        return Ok(false);
    }
    let op = sqlx::query_as::<_, Operation>(
        "SELECT * FROM hosting_backup_operations WHERE id=?",
    )
    .bind(op_id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_error)?;
    notify_changed(state, &op.instance_id, false).await?;
    let result = if op.operation_type == "create" {
        let id = Uuid::parse_str(&op.backup_id).map_err(|_| {
            ApiError::Internal("invalid queued backup ID".into())
        })?;
        backup_service::create_backup_with_id(
            state,
            &op.instance_id,
            "manual",
            Some(op.name),
            id,
        )
        .await
        .map(|_| ())
    } else {
        backup_service::restore_backup_with_safety_name(
            state,
            &op.instance_id,
            &op.backup_id,
            op.safety_name.unwrap_or_else(|| "pre-restore".into()),
        )
        .await
    };
    let files_changed = result.is_ok() && op.operation_type == "restore";
    let (status, error) = match result {
        Ok(()) => ("completed", None),
        Err(error) => ("failed", Some(error.to_string())),
    };
    sqlx::query("UPDATE hosting_backup_operations SET state=?, error=?, completed_at=? WHERE id=?")
        .bind(status).bind(error).bind(chrono::Utc::now().to_rfc3339()).bind(op_id).execute(&state.pool).await.map_err(db_error)?;
    Ok(files_changed)
}

async fn create(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Extension(queue): Extension<Arc<Queue>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<Name>,
) -> ApiResult {
    let id = authorize(&user, &state, &queue, id, wid).await?;
    let bid = Uuid::new_v4().to_string();
    enqueue(
        state,
        queue,
        id,
        bid.clone(),
        name(body.name)?,
        "create",
        user.0.sub,
        None,
    )
    .await?;
    Ok(Json(json!({"id": bid})))
}

async fn restore(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Extension(queue): Extension<Arc<Queue>>,
    Path((id, wid, bid)): Path<(String, String, String)>,
    Json(body): Json<Name>,
) -> ApiResult {
    let id = authorize(&user, &state, &queue, id, wid).await?;
    let r = record(&state, &user, &id, "server:backups").await?;
    if state.instances.contains_key(&r.id) {
        return Err(ApiError::Conflict(
            "Stop the server before restoring a backup".into(),
        ));
    }
    let backup = backup_service::list_backups(&state, &id)
        .await?
        .into_iter()
        .find(|b| b.id == bid)
        .ok_or_else(|| ApiError::NotFound("backup not found".into()))?;
    enqueue(
        state,
        queue,
        id,
        bid,
        backup.name,
        "restore",
        user.0.sub,
        Some(name(body.name)?),
    )
    .await?;
    Ok(Json(json!({})))
}

async fn history_action(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Extension(queue): Extension<Arc<Queue>>,
    Path((id, wid, kind, operation, action)): Path<(
        String,
        String,
        String,
        i64,
        String,
    )>,
) -> ApiResult {
    let id = authorize(&user, &state, &queue, id, wid).await?;
    let op = operations(&state, &id)
        .await?
        .into_iter()
        .find(|op| op.id == operation && op.operation_type == kind)
        .ok_or_else(|| {
            ApiError::NotFound("backup operation not found".into())
        })?;
    match action.as_str() {
        "ack" => {
            sqlx::query("UPDATE hosting_backup_operations SET acknowledged=1 WHERE id=?").bind(op.id).execute(&state.pool).await.map_err(db_error)?;
        }
        "cancel" => {
            let changed = sqlx::query("UPDATE hosting_backup_operations SET state='cancelled', completed_at=? WHERE id=? AND state='pending'")
                .bind(chrono::Utc::now().to_rfc3339()).bind(op.id).execute(&state.pool).await.map_err(db_error)?;
            if changed.rows_affected() == 0 && op.state != "cancelled" {
                return Err(ApiError::Conflict("This backup operation has started or finished and cannot be interrupted safely".into()));
            }
        }
        _ => {
            return Err(ApiError::NotFound(
                "backup operation action not found".into(),
            ))
        }
    }
    notify_changed(&state, &id, false).await?;
    Ok(Json(json!({})))
}

async fn retry(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Extension(queue): Extension<Arc<Queue>>,
    Path((id, wid, bid)): Path<(String, String, String)>,
) -> ApiResult {
    let id = authorize(&user, &state, &queue, id, wid).await?;
    let ops = operations(&state, &id).await?;
    let op =
        ops.into_iter()
            .find(|op| op.backup_id == bid)
            .ok_or_else(|| {
                ApiError::NotFound("backup operation not found".into())
            })?;
    if !matches!(op.state.as_str(), "failed" | "cancelled") {
        return Err(ApiError::Conflict(
            "Only a failed or cancelled backup operation can be retried".into(),
        ));
    }
    sqlx::query(
        "UPDATE hosting_backup_operations SET acknowledged=1 WHERE id=?",
    )
    .bind(op.id)
    .execute(&state.pool)
    .await
    .map_err(db_error)?;
    enqueue(
        state,
        queue,
        id,
        bid,
        op.name,
        &op.operation_type,
        user.0.sub,
        op.safety_name,
    )
    .await?;
    Ok(Json(json!({})))
}

async fn remove_one(
    state: &Arc<AppState>,
    id: &str,
    bid: &str,
) -> Result<(), ApiError> {
    let ops = operations(state, id).await?;
    if ops.iter().any(|op| {
        op.backup_id == bid
            && matches!(op.state.as_str(), "pending" | "ongoing")
    }) {
        return Err(ApiError::Conflict("Cancel or finish this backup operation before deleting its snapshot".into()));
    }
    let exists = backup_service::list_backups(state, id)
        .await?
        .iter()
        .any(|b| b.id == bid);
    if exists {
        backup_service::delete_backup(state, id, bid).await?;
    } else if !ops.iter().any(|op| op.backup_id == bid) {
        return Err(ApiError::NotFound("backup not found".into()));
    }
    sqlx::query("DELETE FROM hosting_backup_operations WHERE instance_id=? AND backup_id=?").bind(id).bind(bid).execute(&state.pool).await.map_err(db_error)?;
    Ok(())
}

async fn remove(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Extension(queue): Extension<Arc<Queue>>,
    Path((id, wid, bid)): Path<(String, String, String)>,
) -> ApiResult {
    let id = authorize(&user, &state, &queue, id, wid).await?;
    let lock = queue.servers.entry(id.clone()).or_default().clone();
    let _guard = lock.lock().await;
    remove_one(&state, &id, &bid).await?;
    notify_changed(&state, &id, false).await?;
    Ok(Json(json!({})))
}

async fn delete_many(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Extension(queue): Extension<Arc<Queue>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<crate::presentation::hosting::models::DeleteBackups>,
) -> ApiResult {
    let id = authorize(&user, &state, &queue, id, wid).await?;
    let lock = queue.servers.entry(id.clone()).or_default().clone();
    let _guard = lock.lock().await;
    for bid in body.backup_ids {
        remove_one(&state, &id, &bid).await?;
    }
    notify_changed(&state, &id, false).await?;
    Ok(Json(json!({})))
}
