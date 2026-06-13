use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{state::AppState, stats_service::get_stats},
    presentation::{
        authz::require_instance_permission, error::ApiError,
        extractors::AuthUser,
    },
};

/// GET /instances/:id/stats
pub async fn get_stats_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_instance_permission(&state, &claims.sub, &id, "server:view")
        .await?;
    let stats = get_stats(&state, &id).await?;
    Ok(Json(json!(stats)))
}
