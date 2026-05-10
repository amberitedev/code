use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        instance_status_service::{kill_instance, restart_instance, send_command, start_instance, stop_instance},
        state::AppState,
    },
    domain::instance::InstanceId,
    presentation::{error::ApiError, extractors::AuthUser},
};

fn parse_id(s: &str) -> Result<InstanceId, ApiError> {
    s.parse::<InstanceId>()
        .map_err(|_| ApiError::BadRequest("invalid instance id".into()))
}

/// POST /instances/:id/start
pub async fn start(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    start_instance(&state, &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/stop
pub async fn stop(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    stop_instance(&state, &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/kill
pub async fn kill(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    kill_instance(&state, &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/restart — stop then start (polls until stopped, 30s timeout).
pub async fn restart(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    restart_instance(&state, &iid).await
        .map_err(|e| {
            if e.to_string().contains("timed out") {
                ApiError::Internal("Shutdown timed out".into())
            } else {
                ApiError::from(e)
            }
        })?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct CommandBody {
    pub command: String,
}

/// POST /instances/:id/command
pub async fn send_command_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CommandBody>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    send_command(&state, &iid, body.command).await?;
    Ok(Json(json!({ "ok": true })))
}
