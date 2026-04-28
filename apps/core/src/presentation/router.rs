use std::sync::Arc;

use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    application::state::AppState,
    presentation::handlers::{
        console, diagnostics, instance_control, instances, logs, macros, modpack, mods,
        properties, setup, stats,
    },
};

/// Build the full Axum router with all routes wired to handlers.
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // System
        .route("/health", get(diagnostics::health))
        .route("/version", get(diagnostics::version))
        .route("/java", get(diagnostics::java_installations))
        // First-run pairing
        .route("/setup", post(setup::complete_setup))
        .route("/setup/status", get(setup::setup_status))
        // WebSocket ticket
        .route("/ws-token", post(console::issue_ws_token))
        // Instances CRUD
        .route("/instances", get(instances::list_instances))
        .route("/instances", post(instances::create_instance))
        .route("/instances/:id", get(instances::get_instance))
        .route("/instances/:id", delete(instances::delete_instance))
        // Instance lifecycle
        .route("/instances/:id/start", post(instance_control::start))
        .route("/instances/:id/stop", post(instance_control::stop))
        .route("/instances/:id/kill", post(instance_control::kill))
        .route("/instances/:id/restart", post(instance_control::restart))
        .route("/instances/:id/command", post(instance_control::send_command_handler))
        // Console (WS) + creation progress (SSE)
        .route("/instances/:id/console", get(console::ws_console))
        .route("/instances/:id/progress", get(console::sse_progress))
        // Modpack install / get / remove / export
        .route("/instances/:id/modpack", post(modpack::install_modpack))
        .route("/instances/:id/modpack", get(modpack::get_modpack))
        .route("/instances/:id/modpack", delete(modpack::remove_modpack))
        .route("/instances/:id/modpack/export", get(modpack::export_modpack_handler))
        // Macros
        .route("/instances/:id/macros", get(macros::list_macros_handler))
        .route("/instances/:id/macros", post(macros::spawn_macro_handler))
        .route("/instances/:id/macros/:pid", delete(macros::kill_macro_handler))
        // Mods
        .route("/instances/:id/mods", get(mods::list_mods_handler))
        .route("/instances/:id/mods", post(mods::add_mod_handler))
        .route("/instances/:id/mods/upload", post(mods::upload_mod_handler))
        .route("/instances/:id/mods/update-all", post(mods::update_all_handler))
        .route("/instances/:id/mods/:filename", delete(mods::delete_mod_handler))
        .route("/instances/:id/mods/:filename", patch(mods::toggle_mod_handler))
        .route("/instances/:id/mods/:filename/update", put(mods::update_mod_handler))
        // Logs
        .route("/instances/:id/logs", get(logs::list_logs_handler))
        .route("/instances/:id/logs/:filename", get(logs::read_log_handler))
        .route("/instances/:id/crash-reports", get(logs::list_crash_reports_handler))
        .route("/instances/:id/crash-reports/:filename", get(logs::read_crash_report_handler))
        // Server properties
        .route("/instances/:id/properties", get(properties::get_properties_handler))
        .route("/instances/:id/properties", patch(properties::patch_properties_handler))
        // Stats
        .route("/instances/:id/stats", get(stats::get_stats_handler))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
