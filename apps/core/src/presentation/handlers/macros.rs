use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        macro_service::{kill_macro, list_macro_files, list_macros, spawn_macro},
        state::AppState,
    },
    domain::instance::InstanceId,
    infrastructure::macro_engine::executor::MacroPid,
    presentation::{error::ApiError, extractors::AuthUser},
};

fn parse_id(s: &str) -> Result<InstanceId, ApiError> {
    s.parse::<InstanceId>()
        .map_err(|_| ApiError::BadRequest("invalid instance id".into()))
}

/// GET /instances/:id/macros — list available files + running PIDs.
pub async fn list_macros_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    let files = list_macro_files(&state, &iid).await;
    let running_pids = list_macros(&state);
    Ok(Json(json!({
        "files": files,
        "running_pids": running_pids,
    })))
}

#[derive(Deserialize)]
pub struct SpawnBody {
    pub name: String,
}

/// POST /instances/:id/macros — spawn a macro by name.
pub async fn spawn_macro_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<SpawnBody>,
) -> Result<Json<Value>, ApiError> {
    let iid = parse_id(&id)?;
    let pid = spawn_macro(&state, iid, body.name)?;
    Ok(Json(json!({ "pid": pid })))
}

/// DELETE /instances/:id/macros/:pid — kill a running macro.
pub async fn kill_macro_handler(
    _auth: AuthUser,
    Path((id, pid_str)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let _iid = parse_id(&id)?;
    let pid: MacroPid = pid_str
        .parse()
        .map_err(|_| ApiError::BadRequest("invalid pid".into()))?;
    kill_macro(&state, pid)?;
    Ok(Json(json!({ "ok": true })))
}
