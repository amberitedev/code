use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        rcon_service::{enable_rcon, execute_command},
        state::AppState,
    },
    presentation::{
        error::ApiError, extractors::AuthUser,
        instance_path::resolve_authorized_instance_id,
    },
};

#[derive(Debug, Deserialize)]
pub struct RconCommandRequest {
    pub command: String,
}

/// POST /instances/:id/rcon
///
/// Execute a single RCON command against a running instance and return the
/// raw server response.
pub async fn execute_rcon_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<RconCommandRequest>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_authorized_instance_id(
        &state,
        &claims.sub,
        &id,
        "server:console",
    )
    .await?
    .to_string();
    let command = body.command.trim();
    if command.is_empty() {
        return Err(ApiError::BadRequest("command must not be empty".into()));
    }
    let response = execute_command(&state, &instance_id, command).await?;
    Ok(Json(json!({ "response": response })))
}

/// POST /instances/:id/rcon/enable
///
/// Enable RCON on an instance by patching `server.properties` (generating a
/// password if one is absent). The response reports whether a restart is
/// required for the change to take effect.
pub async fn enable_rcon_handler(
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
    let result = enable_rcon(&state, &instance_id).await?;
    Ok(Json(json!(result)))
}
