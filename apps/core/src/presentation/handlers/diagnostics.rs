use std::sync::Arc;

use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::application::state::AppState;

/// GET /health — liveness probe.
pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// GET /version — package version info.
pub async fn version() -> Json<Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
    }))
}

/// GET /java — list detected Java installations.
pub async fn java_installations(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT version, path FROM java_installations ORDER BY version DESC")
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();

    let installations: Vec<Value> = rows
        .into_iter()
        .map(|(version, path)| json!({ "version": version, "path": path }))
        .collect();

    Json(json!({ "installations": installations }))
}
