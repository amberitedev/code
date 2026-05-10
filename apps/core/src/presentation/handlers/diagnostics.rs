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

/// GET /java — list detected Java installations (ARCH-08: via JavaStore port).
pub async fn java_installations(State(state): State<Arc<AppState>>) -> Json<Value> {
    let installs = state.java_store.list_all().await;
    let installations: Vec<Value> = installs
        .into_iter()
        .map(|j| json!({ "version": j.version, "path": j.path.display().to_string() }))
        .collect();
    Json(json!({ "installations": installations }))
}
