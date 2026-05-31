use std::sync::Arc;

use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::{
    api::{
        ConnectionHandshakeRequest, ConnectionHandshakeResponse,
        ConnectionRejectReason, HANDSHAKE_PROTOCOL,
    },
    application::state::AppState,
};

/// GET /health — liveness probe.
pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// POST /connection/handshake — app ↔ Core nonce handshake.
pub async fn connection_handshake(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConnectionHandshakeRequest>,
) -> Json<ConnectionHandshakeResponse> {
    let reason = if body.protocol != HANDSHAKE_PROTOCOL {
        Some(ConnectionRejectReason::ProtocolMismatch)
    } else if body
        .known_core_id
        .as_deref()
        .is_some_and(|id| id != state.core_id)
    {
        Some(ConnectionRejectReason::WrongCore)
    } else {
        None
    };

    Json(ConnectionHandshakeResponse {
        nonce: body.nonce,
        ok: reason.is_none(),
        core_id: state.core_id.clone(),
        protocol: HANDSHAKE_PROTOCOL,
        version: env!("CARGO_PKG_VERSION"),
        reason,
    })
}

/// GET /version — package version info.
pub async fn version() -> Json<Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
    }))
}

/// GET /java — list detected Java installations (ARCH-08: via JavaStore port).
pub async fn java_installations(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let installs = state.java_store.list_all().await;
    let installations: Vec<Value> = installs
        .into_iter()
        .map(|j| json!({ "version": j.version, "path": j.path.display().to_string() }))
        .collect();
    Json(json!({ "installations": installations }))
}
