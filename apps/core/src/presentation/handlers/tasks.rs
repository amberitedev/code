use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{
        state::AppState,
        task_service::{
            create_task, delete_task, get_task, list_tasks, update_task,
            CreateTaskBody, UpdateTaskBody,
        },
    },
    presentation::{
        error::ApiError, extractors::AuthUser,
        instance_path::resolve_authorized_instance_id,
    },
};

/// GET /instances/:id/tasks
pub async fn list_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_authorized_instance_id(
        &state,
        &claims.sub,
        &id,
        "server:settings",
    )
    .await?
    .to_string();
    let tasks = list_tasks(&state, &instance_id).await?;
    Ok(Json(json!({ "tasks": tasks })))
}

/// POST /instances/:id/tasks
pub async fn create_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateTaskBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_authorized_instance_id(
        &state,
        &claims.sub,
        &id,
        "server:settings",
    )
    .await?
    .to_string();
    let task = create_task(&state, &instance_id, body).await?;
    Ok(Json(json!(task)))
}

/// GET /instances/:id/tasks/:task_id
pub async fn get_handler(
    AuthUser(claims): AuthUser,
    Path((id, task_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_authorized_instance_id(
        &state,
        &claims.sub,
        &id,
        "server:settings",
    )
    .await?
    .to_string();
    let task = get_task(&state, &instance_id, &task_id).await?;
    Ok(Json(json!(task)))
}

/// PATCH /instances/:id/tasks/:task_id
pub async fn update_handler(
    AuthUser(claims): AuthUser,
    Path((id, task_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateTaskBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_authorized_instance_id(
        &state,
        &claims.sub,
        &id,
        "server:settings",
    )
    .await?
    .to_string();
    let task = update_task(&state, &instance_id, &task_id, body).await?;
    Ok(Json(json!(task)))
}

/// DELETE /instances/:id/tasks/:task_id
pub async fn delete_handler(
    AuthUser(claims): AuthUser,
    Path((id, task_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_authorized_instance_id(
        &state,
        &claims.sub,
        &id,
        "server:settings",
    )
    .await?
    .to_string();
    delete_task(&state, &instance_id, &task_id).await?;
    Ok(Json(json!({ "ok": true })))
}
