use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{state::AppState, stats_service::get_stats},
    presentation::{
        error::ApiError,
        extractors::AuthUser,
        instance_path::resolve_authorized_instance_id,
    },
};

/// GET /instances/:id/stats
pub async fn get_stats_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_authorized_instance_id(&state, &claims.sub, &id, "server:view")
            .await?
            .to_string();
    let stats = get_stats(&state, &instance_id).await?;
    Ok(Json(json!(stats)))
}
