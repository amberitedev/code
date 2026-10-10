use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{modpack_service::remove, state::AppState},
    presentation::{error::ApiError, instance_path::resolve_instance_id},
};

/// DELETE /instances/:id/modpack — remove the modpack manifest.
pub async fn remove_modpack(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_instance_id(&state, &id).await?.to_string();
    remove(&state, &instance_id).await?;
    Ok(Json(json!({ "ok": true })))
}
