use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        mod_service::{
            add_mod, delete_mod, list_mods, toggle_mod, update_all_mods, update_mod, upload_mod,
        },
        state::AppState,
    },
    presentation::{error::ApiError, extractors::AuthUser},
};

#[derive(Deserialize)]
pub struct AddModBody {
    pub version_id: String,
}

#[derive(Deserialize)]
pub struct ToggleBody {
    pub enabled: bool,
}

/// GET /instances/:id/mods
pub async fn list_mods_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let mods = list_mods(&state, &id).await?;
    Ok(Json(json!({ "mods": mods })))
}

/// POST /instances/:id/mods — add mod from Modrinth version ID.
pub async fn add_mod_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddModBody>,
) -> Result<Json<Value>, ApiError> {
    let info = add_mod(&state, &id, &body.version_id).await?;
    Ok(Json(json!(info)))
}

/// POST /instances/:id/mods/upload — upload a jar directly.
pub async fn upload_mod_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
    {
        let filename = field
            .file_name()
            .unwrap_or("mod.jar")
            .to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        upload_mod(&state, &id, &filename, data).await?;
        return Ok(Json(json!({ "ok": true, "filename": filename })));
    }
    Err(ApiError::BadRequest("no file provided".into()))
}

/// DELETE /instances/:id/mods/:filename
pub async fn delete_mod_handler(
    _auth: AuthUser,
    Path((id, filename)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let running = delete_mod(&state, &id, &filename).await?;
    Ok(Json(json!({ "ok": true, "restart_required": running })))
}

/// PATCH /instances/:id/mods/:filename — toggle enabled/disabled.
pub async fn toggle_mod_handler(
    _auth: AuthUser,
    Path((id, filename)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ToggleBody>,
) -> Result<Json<Value>, ApiError> {
    toggle_mod(&state, &id, &filename, body.enabled).await?;
    Ok(Json(json!({ "ok": true })))
}

/// PUT /instances/:id/mods/:filename/update — update to latest version.
pub async fn update_mod_handler(
    _auth: AuthUser,
    Path((id, filename)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let updated = update_mod(&state, &id, &filename).await?;
    Ok(Json(json!({ "updated": updated })))
}

/// POST /instances/:id/mods/update-all
pub async fn update_all_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let result = update_all_mods(&state, &id).await?;
    Ok(Json(json!(result)))
}
