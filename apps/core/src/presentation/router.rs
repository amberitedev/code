//! Router - Assembles Axum web server.

use crate::application::registry::ServiceRegistry;
use crate::presentation::handlers::{auth_api, diagnostics_api, instance_api, websockets};
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

/// Create router.
pub fn create_router(registry: ServiceRegistry) -> Router {
    let registry = Arc::new(registry);

    Router::new()
        // Health check
        .route("/health", get(diagnostics_api::health_check))
        // System stats
        .route("/stats", get(diagnostics_api::get_system_stats))
        // Auth routes
        .route("/login", post(auth_api::login))
        .route("/setup", post(auth_api::setup))
        // Instance routes
        .route("/instances/:id/start", post(instance_api::start_instance))
        .route("/instances/:id/stop", post(instance_api::stop_instance))
        .route("/instances/:id/kill", post(instance_api::kill_instance))
        .route("/instances/:id/command", post(instance_api::send_command))
        // WebSocket routes
        .route("/instances/:id/console", get(websockets::ws_handler))
        // State
        .with_state(registry)
}
