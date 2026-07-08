use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{installation_service::installation_dir, state::AppState},
    domain::server_installation::{InstallationId, ServerInstallationRecord},
    presentation::{
        authz::require_core_manager, error::ApiError, extractors::AuthUser,
    },
};

fn record_json(r: &ServerInstallationRecord) -> Value {
    json!({
        "id": r.id.to_string(),
        "game_version": r.game_version,
        "loader": r.loader.to_string(),
        "loader_version": r.loader_version,
        "status": r.status.to_string(),
        "error": r.error,
        "created_at": r.created_at,
        "updated_at": r.updated_at,
    })
}

/// GET /installations — list all shared server installations.
pub async fn list_installations(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_core_manager(&state, &claims.sub).await?;
    let records = state.installation_store.list().await?;
    let installations: Vec<Value> = records.iter().map(record_json).collect();
    Ok(Json(json!({ "installations": installations })))
}

/// GET /installations/:id — get a single shared installation.
pub async fn get_installation(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_core_manager(&state, &claims.sub).await?;
    let iid = InstallationId(id);
    let record =
        state.installation_store.get(&iid).await?.ok_or_else(|| {
            ApiError::NotFound("installation not found".into())
        })?;
    Ok(Json(record_json(&record)))
}

/// DELETE /installations/:id — delete a shared installation and its files.
///
/// Rejected while any instance is still bound to it, since the shared files
/// would be removed out from under those instances.
pub async fn delete_installation(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_core_manager(&state, &claims.sub).await?;
    let iid = InstallationId(id);
    let bound = state
        .instance_store
        .list_by_installation(&iid.to_string())
        .await?;
    if !bound.is_empty() {
        return Err(ApiError::Conflict(format!(
            "{} instance(s) still use this installation",
            bound.len()
        )));
    }

    let dir = installation_dir(&state, &iid);
    let _ = tokio::fs::remove_dir_all(&dir).await;
    state.installation_store.delete(&iid).await?;
    Ok(Json(json!({ "ok": true })))
}
