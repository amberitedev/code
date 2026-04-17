//! Router - Assembles Axum web server.

use crate::application::registry::ServiceRegistry;
use crate::infrastructure::supabase_auth::SupabaseJwtValidator;
use crate::presentation::handlers::{auth_api, diagnostics_api, instance_api, websockets};
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

pub fn create_router(registry: ServiceRegistry, jwt_validator: SupabaseJwtValidator) -> Router {
    let registry = Arc::new(registry);
    let jwt_validator = Arc::new(jwt_validator);

    let app = Router::new()
        // Health check (public)
        .route("/health", get(diagnostics_api::health_check))
        // System stats (public for now)
        .route("/stats", get(diagnostics_api::get_system_stats))
        // Auth routes (public)
        .route("/login", post(auth_api::login))
        .route("/setup", post(auth_api::setup))
        // Instance routes (protected)
        .route("/instances", get(instance_api::list_instances))
        .route("/instances/:id/start", post(instance_api::start_instance))
        .route("/instances/:id/stop", post(instance_api::stop_instance))
        .route("/instances/:id/kill", post(instance_api::kill_instance))
        .route("/instances/:id/command", post(instance_api::send_command))
        // WebSocket routes (auth via query param)
        .route("/instances/:id/console", get(websockets::ws_handler));

    app.with_state((registry, jwt_validator))
}
