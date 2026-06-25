use std::sync::Arc;

use axum::{extract::State, Json};
use serde_json::json;

use crate::{
    application::{core_projection_service, state::AppState},
    presentation::{
        authz::require_core_manager, error::ApiError, extractors::AuthUser,
    },
};

pub async fn resync(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_core_manager(&state, &claims.sub).await?;
    let result = core_projection_service::sync_projection(&state)
        .await
        .map_err(|error| ApiError::ServiceUnavailable(error.to_string()))?;
    Ok(Json(json!(result)))
}
