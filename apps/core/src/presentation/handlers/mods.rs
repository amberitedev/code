use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        mod_service::{add_mod, add_mod_project, delete_mod, toggle_mod},
        state::AppState,
    },
    presentation::{error::ApiError, instance_path::resolve_instance_id},
};

#[derive(Deserialize)]
pub struct AddModBody {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ToggleBody {
    pub enabled: bool,
}

/// POST /instances/:id/mods — add mod from Modrinth version ID.
pub async fn add_mod_handler(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddModBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_instance_id(&state, &id).await?.to_string();
    let info = if let Some(version_id) = body.version_id {
        add_mod(&state, &instance_id, &version_id).await?
    } else if let Some(project_id) = body.project_id {
        add_mod_project(&state, &instance_id, &project_id).await?
    } else {
        return Err(ApiError::BadRequest(
            "version_id or project_id is required".into(),
        ));
    };
    Ok(Json(json!(info)))
}

/// DELETE /instances/:id/mods/:filename
pub async fn delete_mod_handler(
    Path((id, filename)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_instance_id(&state, &id).await?.to_string();
    let running = delete_mod(&state, &instance_id, &filename).await?;
    Ok(Json(json!({ "ok": true, "restart_required": running })))
}

/// PATCH /instances/:id/mods/:filename — toggle enabled/disabled.
pub async fn toggle_mod_handler(
    Path((id, filename)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ToggleBody>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_instance_id(&state, &id).await?.to_string();
    toggle_mod(&state, &instance_id, &filename, body.enabled).await?;
    Ok(Json(json!({ "ok": true })))
}
