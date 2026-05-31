use std::sync::Arc;

use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method,
    },
    routing::{delete, get, patch, post, put},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    application::state::AppState,
    presentation::handlers::{
        backups, console, diagnostics, events, fs, instance_control, instances,
        logs, macros, modpack, mods, properties, relay, setup, social, stats,
        sync,
    },
};

/// Build the full Axum router with all routes wired to handlers.
pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = cors_layer(&state);

    Router::new()
        // System
        .route("/health", get(diagnostics::health))
        .route(
            "/connection/handshake",
            post(diagnostics::connection_handshake),
        )
        .route("/version", get(diagnostics::version))
        .route("/java", get(diagnostics::java_installations))
        // First-run pairing
        .route("/setup", post(setup::complete_setup))
        .route("/setup/status", get(setup::setup_status))
        // Core-local relay (Mode 2)
        .route("/relay/messages", post(relay::publish))
        .route("/relay/messages/status/:id", get(relay::status))
        .route("/relay/messages/:recipient_id", get(relay::pending))
        .route("/relay/messages/:id/ack", post(relay::ack))
        .route("/relay/messages/:id/complete", post(relay::complete))
        // Core social, permissions, and sync scaffolding
        .route("/core", get(social::get_core))
        .route("/core", patch(social::update_core))
        .route("/core/members", get(social::list_members))
        .route("/core/members", post(social::upsert_member))
        .route("/core/members/:user_id", delete(social::remove_member))
        .route("/core/bans", get(social::list_bans))
        .route("/core/bans", post(social::ban_member))
        .route("/sync/profiles", get(sync::list_sync_profiles))
        .route("/sync/profiles", post(sync::register_sync_profile))
        .route(
            "/sync/profiles/from-mrpack",
            post(sync::create_sync_profile_from_mrpack),
        )
        .route(
            "/sync/profiles/:profile_id",
            delete(sync::remove_sync_profile),
        )
        .route(
            "/sync/profiles/:profile_id/check-version",
            get(sync::check_sync_version),
        )
        .route(
            "/sync/profiles/:profile_id/snapshots",
            get(sync::list_snapshots),
        )
        .route(
            "/sync/profiles/:profile_id/snapshots",
            post(sync::publish_snapshot),
        )
        .route(
            "/sync/profiles/:profile_id/snapshots/:snapshot_id/download",
            get(sync::download_snapshot),
        )
        .route(
            "/sync/profiles/:profile_id/events",
            get(sync::list_sync_events),
        )
        // WebSocket ticket
        .route("/ws-token", post(console::issue_ws_token))
        .route("/events", get(events::stream_events))
        // Instances CRUD
        .route("/instances", get(instances::list_instances))
        .route("/instances", post(instances::create_instance))
        .route("/instances/:id", get(instances::get_instance))
        .route("/instances/:id", patch(instances::patch_instance))
        .route("/instances/:id", delete(instances::delete_instance))
        .route("/instances/:id/startup", get(instances::get_startup))
        // Instance lifecycle
        .route("/instances/:id/start", post(instance_control::start))
        .route("/instances/:id/stop", post(instance_control::stop))
        .route("/instances/:id/kill", post(instance_control::kill))
        .route("/instances/:id/restart", post(instance_control::restart))
        .route("/instances/:id/repair", post(instance_control::repair))
        .route(
            "/instances/:id/change-version",
            post(instance_control::change_version_handler),
        )
        .route(
            "/instances/:id/command",
            post(instance_control::send_command_handler),
        )
        // Console (WS) + creation progress (SSE)
        .route("/instances/:id/console", get(console::ws_console))
        .route("/instances/:id/progress", get(console::sse_progress))
        // Modpack install / get / remove / export
        .route("/instances/:id/modpack", post(modpack::install_modpack))
        .route(
            "/instances/:id/modpack/modrinth",
            post(modpack::install_modpack_modrinth),
        )
        .route("/instances/:id/modpack", get(modpack::get_modpack))
        .route("/instances/:id/modpack", delete(modpack::remove_modpack))
        .route(
            "/instances/:id/modpack/export",
            get(modpack::export_modpack_handler),
        )
        // Macros
        .route("/instances/:id/macros", get(macros::list_macros_handler))
        .route("/instances/:id/macros", post(macros::spawn_macro_handler))
        .route(
            "/instances/:id/macros/:pid",
            delete(macros::kill_macro_handler),
        )
        // Mods
        .route("/instances/:id/mods", get(mods::list_mods_handler))
        .route("/instances/:id/mods", post(mods::add_mod_handler))
        .route("/instances/:id/mods/upload", post(mods::upload_mod_handler))
        .route(
            "/instances/:id/mods/update-all",
            post(mods::update_all_handler),
        )
        .route(
            "/instances/:id/mods/:filename",
            delete(mods::delete_mod_handler),
        )
        .route(
            "/instances/:id/mods/:filename",
            patch(mods::toggle_mod_handler),
        )
        .route(
            "/instances/:id/mods/:filename/update",
            put(mods::update_mod_handler),
        )
        // Logs
        .route("/instances/:id/logs", get(logs::list_logs_handler))
        .route("/instances/:id/logs/:filename", get(logs::read_log_handler))
        .route(
            "/instances/:id/crash-reports",
            get(logs::list_crash_reports_handler),
        )
        .route(
            "/instances/:id/crash-reports/:filename",
            get(logs::read_crash_report_handler),
        )
        // Server properties
        .route(
            "/instances/:id/properties",
            get(properties::get_properties_handler),
        )
        .route(
            "/instances/:id/properties",
            patch(properties::patch_properties_handler),
        )
        // Stats
        .route("/instances/:id/stats", get(stats::get_stats_handler))
        // Filesystem
        .route("/instances/:id/fs", get(fs::list_handler))
        .route("/instances/:id/fs", delete(fs::delete_handler))
        .route("/instances/:id/fs/download", get(fs::download_handler))
        .route("/instances/:id/fs/upload", post(fs::upload_handler))
        .route("/instances/:id/fs/read", get(fs::read_handler))
        .route("/instances/:id/fs/write", put(fs::write_handler))
        .route("/instances/:id/fs/create", post(fs::create_file_handler))
        .route("/instances/:id/fs/mkdir", post(fs::mkdir_handler))
        .route("/instances/:id/fs/move", put(fs::move_handler))
        .route("/instances/:id/fs/unzip", post(fs::unzip_handler))
        .route("/instances/:id/fs/zip", post(fs::zip_handler))
        .route("/instances/:id/fs/copy", post(fs::copy_handler))
        .route("/instances/:id/fs/url", get(fs::url_handler))
        .route("/instances/:id/fs/search", get(fs::search_handler))
        .route("/fs/file/:key", get(fs::download_by_key_handler))
        // Backups — delete-many and schedule registered before /:bid to prevent literal-string capture
        .route(
            "/instances/:id/backups/delete-many",
            post(backups::delete_many_handler),
        )
        .route(
            "/instances/:id/backups/schedule",
            get(backups::get_schedule_handler),
        )
        .route(
            "/instances/:id/backups/schedule",
            put(backups::set_schedule_handler),
        )
        .route("/instances/:id/backups", get(backups::list_handler))
        .route("/instances/:id/backups", post(backups::create_handler))
        .route(
            "/instances/:id/backups/:bid",
            delete(backups::delete_handler),
        )
        .route(
            "/instances/:id/backups/:bid",
            patch(backups::rename_handler),
        )
        .route(
            "/instances/:id/backups/:bid/lock",
            patch(backups::lock_handler),
        )
        .route(
            "/instances/:id/backups/:bid/restore",
            post(backups::restore_handler),
        )
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

fn cors_layer(state: &AppState) -> CorsLayer {
	if state.config.dev_mode || state.config.allowed_origin == "*" {
		return CorsLayer::new()
			.allow_origin(Any)
			.allow_methods(Any)
			.allow_headers(Any);
	}

	let origin = state
		.config
		.allowed_origin
		.parse::<HeaderValue>()
		.unwrap_or_else(|_| HeaderValue::from_static("https://amberite.dev"));

	CorsLayer::new()
		.allow_origin(origin)
		.allow_methods([
			Method::GET,
			Method::POST,
			Method::PUT,
			Method::PATCH,
			Method::DELETE,
		])
		.allow_headers([AUTHORIZATION, CONTENT_TYPE])
		.allow_credentials(true)
}
