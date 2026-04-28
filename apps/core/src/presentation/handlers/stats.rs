use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{state::AppState, stats_service::get_stats},
    presentation::{error::ApiError, extractors::AuthUser},
};

/// GET /instances/:id/stats
pub async fn get_stats_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let stats = get_stats(&state, &id).await?;
    Ok(Json(json!(stats)))
}
