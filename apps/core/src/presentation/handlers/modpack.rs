use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{
        export_service::export_modpack,
        modpack_service::{get_manifest, install, remove},
        state::AppState,
    },
    domain::modpack::ModpackManifest,
    presentation::{error::ApiError, extractors::AuthUser},
};

/// POST /instances/:id/modpack — upload and install a `.mrpack` file.
pub async fn install_modpack(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    let mut mrpack_bytes: Option<bytes::Bytes> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
    {
        if field.name() == Some("mrpack") {
            mrpack_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(e.to_string()))?,
            );
        }
    }

    let data =
        mrpack_bytes.ok_or_else(|| ApiError::BadRequest("missing 'mrpack' field".into()))?;

    // Write to a temp file for the installer to read
    let tmp =
        tempfile::NamedTempFile::new().map_err(|e| ApiError::Internal(e.to_string()))?;
    tokio::fs::write(tmp.path(), &data)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let manifest = install(&state, &id, tmp.path()).await?;
    Ok(Json(manifest_to_value(&manifest)))
}

/// GET /instances/:id/modpack — get the installed modpack manifest.
pub async fn get_modpack(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let manifest = get_manifest(&state, &id)
        .await?
        .ok_or_else(|| ApiError::NotFound("no modpack installed for this instance".into()))?;
    Ok(Json(manifest_to_value(&manifest)))
}

/// DELETE /instances/:id/modpack — remove the modpack manifest.
pub async fn remove_modpack(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    remove(&state, &id).await?;
    Ok(Json(json!({ "ok": true })))
}

/// GET /instances/:id/modpack/export — download instance as `.mrpack`.
pub async fn export_modpack_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
    let (data, filename) = export_modpack(&state, &id).await?;
    let disposition = format!("attachment; filename=\"{filename}\"");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/zip"),
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposition)
                .unwrap_or(HeaderValue::from_static("attachment")),
        )
        .body(Body::from(data))
        .unwrap())
}

fn manifest_to_value(m: &ModpackManifest) -> Value {
    json!({
        "id": m.id,
        "instance_id": m.instance_id,
        "pack_name": m.pack_name,
        "pack_version": m.pack_version,
        "game_version": m.game_version,
        "loader": m.loader,
        "loader_version": m.loader_version,
        "modrinth_project_id": m.modrinth_project_id,
        "modrinth_version_id": m.modrinth_version_id,
        "installed_at": m.installed_at,
    })
}
