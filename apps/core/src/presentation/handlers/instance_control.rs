use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        instance_service::{change_version, repair_instance},
        instance_status_service::{
            kill_instance, restart_instance, start_instance, stop_instance,
        },
        state::AppState,
    },
    domain::instance::ModLoader,
    presentation::{error::ApiError, instance_path::resolve_instance_id},
};

/// POST /instances/:id/start
pub async fn start(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = resolve_instance_id(&state, &id).await?;
    start_instance(&state, &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/stop
pub async fn stop(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = resolve_instance_id(&state, &id).await?;
    stop_instance(&state, &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/kill
pub async fn kill(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = resolve_instance_id(&state, &id).await?;
    kill_instance(&state, &iid).await?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/restart — stop then start (polls until stopped, 30s timeout).
pub async fn restart(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = resolve_instance_id(&state, &id).await?;
    restart_instance(&state, &iid).await.map_err(|e| {
        if e.to_string().contains("timed out") {
            ApiError::Internal("Shutdown timed out".into())
        } else {
            ApiError::from(e)
        }
    })?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /instances/:id/repair — re-download/reinstall the server JAR.
/// Refuses while the instance is running. Returns immediately; track via SSE.
pub async fn repair(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = resolve_instance_id(&state, &id).await?;
    repair_instance(&state, &iid).await?;
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
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChangeVersionBody>,
) -> Result<Json<Value>, ApiError> {
    let iid = resolve_instance_id(&state, &id).await?;
    change_version(
        &state,
        &iid,
        body.game_version,
        body.loader,
        body.loader_version,
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}
