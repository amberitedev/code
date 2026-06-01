use std::sync::Arc;

use axum::{
	extract::{Path, State},
	Json,
};
use serde_json::{json, Value};

use crate::{
	application::{query_service::query_instance, state::AppState},
	presentation::{error::ApiError, extractors::AuthUser},
};

/// GET /instances/:id/query
///
/// Perform a Minecraft Server List Ping against a running instance and return
/// its advertised MOTD, version, player counts and measured latency.
pub async fn query_instance_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	let result = query_instance(&state, &id).await?;
	Ok(Json(json!(result)))
}
