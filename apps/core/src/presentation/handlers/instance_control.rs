use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        access_service, activity_service,
        instance_service::{change_version, repair_instance},
        instance_status_service::{
            kill_instance, restart_instance, send_command, start_instance,
            stop_instance,
        },
        state::AppState,
    },
    domain::instance::{InstanceId, ModLoader},
    presentation::{error::ApiError, extractors::AuthUser},
};

fn parse_id(s: &str) -> Result<InstanceId, ApiError> {
    s.parse::<InstanceId>()
        .map_err(|_| ApiError::BadRequest("invalid instance id".into()))
}

/// POST /instances/:id/start
pub async fn start(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    require_instance_permission(&state, &claims.sub, &id, "server:power")
        .await?;
    start_instance(&state, &iid).await?;
    log_action(&state, &claims.sub, "instance_started", &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/stop
pub async fn stop(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    require_instance_permission(&state, &claims.sub, &id, "server:power")
        .await?;
    stop_instance(&state, &iid).await?;
    log_action(&state, &claims.sub, "instance_stopped", &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/kill
pub async fn kill(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    require_instance_permission(&state, &claims.sub, &id, "server:power")
        .await?;
    kill_instance(&state, &iid).await?;
    log_action(&state, &claims.sub, "instance_killed", &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/restart — stop then start (polls until stopped, 30s timeout).
pub async fn restart(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    require_instance_permission(&state, &claims.sub, &id, "server:power")
        .await?;
    restart_instance(&state, &iid).await.map_err(|e| {
        if e.to_string().contains("timed out") {
            ApiError::Internal("Shutdown timed out".into())
        } else {
            ApiError::from(e)
        }
    })?;
    log_action(&state, &claims.sub, "instance_restarted", &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct CommandBody {
    pub command: String,
}

/// POST /instances/:id/command
pub async fn send_command_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CommandBody>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    require_instance_permission(&state, &claims.sub, &id, "server:power")
        .await?;
    send_command(&state, &iid, body.command).await?;
    log_action(&state, &claims.sub, "console_command_executed", &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/repair — re-download/reinstall the server JAR.
/// Refuses while the instance is running. Returns immediately; track via SSE.
pub async fn repair(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    require_instance_permission(&state, &claims.sub, &id, "server:settings")
        .await?;
    repair_instance(&state, &iid).await?;
    log_action(&state, &claims.sub, "instance_repaired", &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ChangeVersionBody {
    pub game_version: Option<String>,
    pub loader: Option<ModLoader>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub loader_version: Option<Option<String>>,
}

/// Distinguishes absent (`None`) from explicit `null` (`Some(None)`) for loader_version.
fn deserialize_optional_string<'de, D>(
    d: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<String>::deserialize(d)?))
}

/// POST /instances/:id/change-version — change game version/loader, then reinstall.
/// Refuses while the instance is running. Returns immediately; track via SSE.
pub async fn change_version_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChangeVersionBody>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    require_instance_permission(&state, &claims.sub, &id, "server:settings")
        .await?;
    change_version(
        &state,
        &iid,
        body.game_version,
        body.loader,
        body.loader_version,
    )
    .await?;
    log_action(&state, &claims.sub, "instance_version_changed", &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn log_action(
    state: &Arc<AppState>,
    actor_user_id: &str,
    action: &str,
    instance_id: &InstanceId,
) -> Result<(), ApiError> {
    activity_service::record(
        state,
        actor_user_id,
        action,
        Some(&instance_id.to_string()),
        None,
        None,
    )
    .await?;
    Ok(())
}

async fn require_instance_permission(
    state: &Arc<AppState>,
    user_id: &str,
    instance_id: &str,
    permission: &str,
) -> Result<(), ApiError> {
    access_service::require_instance_permission(
        state,
        user_id,
        instance_id,
        permission,
    )
    .await?;
    Ok(())
}
