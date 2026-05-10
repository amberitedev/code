use std::{collections::HashMap, path::PathBuf, sync::Arc};

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::state::AppState,
    domain::instance::InstanceId,
    infrastructure::minecraft::server_properties::{patch_properties, read_properties},
    presentation::{error::ApiError, extractors::AuthUser},
};

/// GET /instances/:id/properties
pub async fn get_properties_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let data_dir = fetch_data_dir(&state, &id).await?;
    let props = read_properties(&data_dir)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({ "properties": props })))
}

/// PATCH /instances/:id/properties — update specific keys in-place.
pub async fn patch_properties_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let data_dir = fetch_data_dir(&state, &id).await?;
    let updated = patch_properties(&data_dir, &body)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({ "updated_keys": updated })))
}

/// SEC-04: parse UUID and use instance_store — no raw SQL.
async fn fetch_data_dir(state: &Arc<AppState>, instance_id: &str) -> Result<PathBuf, ApiError> {
    let iid = instance_id
        .parse::<InstanceId>()
        .map_err(|_| ApiError::BadRequest("invalid instance id — must be a UUID".into()))?;
    let record = state.instance_store.get(&iid).await?;
    Ok(PathBuf::from(&record.data_dir))
}
