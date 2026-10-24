//! The HTTP API: Axum routes, one file per resource, each with typed request and response structs.
//! The same router serves HTTPS and the local socket (see `access::local`).
//!
//! Hosting routes match the `Archon.*` shapes in
//! `packages/api-client/src/modules/archon/types.ts`, which the App already sends. Routes only
//! Amberite has (access, sessions, activity, backend sync) generate their TypeScript from these
//! structs with `ts-rs`; see `typescript`.

pub mod access;
pub mod activity;
pub mod backend;
pub mod backups;
pub mod console;
pub mod content;
pub mod error;
pub mod eula;
pub mod events;
pub mod files;
mod health;
pub mod insight;
pub mod logs;
pub mod pairing;
pub mod performance;
pub mod properties;
pub mod servers;
pub mod sessions;
mod startup;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod typescript;
pub mod update;

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::StatusCode,
    routing::{delete, get, post, put},
};
use color_eyre::eyre::{Result, eyre};
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use sqlx::SqlitePool;
use tokio::io::{AsyncRead, AsyncWrite};
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

/// The longest a request may take, except the routes that wait on a server.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a connection may sit without a request, or take to send one's headers.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Bodies are capped at this; upload routes stream to disk with their own cap.
const MAX_BODY: usize = 10 * 1024 * 1024;

/// What every route can reach.
pub struct AppState {
    pub db: SqlitePool,
    pub servers: Arc<crate::servers::Servers>,
    pub access: Arc<crate::access::Access>,
    pub content: Arc<crate::content::Content>,
    pub backups: Arc<crate::backups::Backups>,
    pub uploads: files::Uploads,
    pub update: Arc<crate::update::Updater>,
    pub network: Arc<crate::network::forward::Network>,
}

/// Every route. Each one checks its caller itself (see `access`); the network listener adds its
/// own checks on top (see `network::tls`).
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health::get))
        .route("/sessions", post(sessions::sign_in).get(sessions::list))
        .route("/sessions/current", delete(sessions::sign_out))
        .route("/sessions/{id}", delete(sessions::revoke))
        .route("/access/me", get(access::me))
        .route(
            "/servers/{id}/access",
            get(access::list).post(access::invite),
        )
        .route(
            "/servers/{id}/access/{user_id}",
            delete(access::remove).patch(access::change_role),
        )
        .route("/activity", get(activity::list))
        .route("/pairing", delete(pairing::remove).get(pairing::get))
        .route("/pairing/code", post(pairing::new_code))
        .route("/update", get(update::get).patch(update::settings))
        .route("/update/check", post(update::check))
        .route("/update/rollback", post(update::rollback))
        .route("/eula", get(eula::get).post(eula::post))
        .route("/logs", get(logs::get))
        .route("/servers/{id}/performance", get(performance::get))
        .route("/servers/{id}/memory", put(performance::set_memory))
        .route("/servers/{id}/restarts", put(performance::set_restarts))
        .route("/servers/{id}/console", post(console::send))
        .route("/servers/{id}/flags", put(performance::set_flags))
        .route("/servers/{id}/insight", get(insight::get))
        .route("/machine", get(insight::machine))
        .route("/modrinth/v0/servers", get(servers::list))
        .route("/modrinth/v0/servers/{id}", get(servers::get))
        .route("/modrinth/v0/servers/{id}/ws", get(servers::ws_auth))
        .route("/servers/{id}/ws", get(events::console))
        .route("/local/servers", post(servers::create))
        .route("/local/servers/{id}", delete(servers::delete))
        .route("/local/servers/{id}/eula", post(servers::accept_eula))
        .route("/v1/servers", get(servers::list_v1))
        .route("/v1/servers/{id}", get(servers::get_v1))
        .route("/modrinth/v0/servers/{id}/fs", get(files::auth))
        .route(
            "/modrinth/v0/servers/{id}/startup",
            get(startup::get_v0).post(startup::update_v0),
        )
        .route(
            "/v1/servers/{id}/worlds/{wid}/options/startup",
            get(startup::get_v1).patch(startup::patch_v1),
        )
        .route("/servers/{id}/java", put(startup::set_java_path))
        .route(
            "/v1/servers/{id}/worlds/{wid}/backups-queue",
            get(backups::list).post(backups::create),
        )
        .route("/v1/servers/{id}/worlds/{wid}/backups-queue/delete-many", post(backups::delete_many))
        .route("/v1/servers/{id}/worlds/{wid}/backups-queue/{bid}", delete(backups::delete))
        .route("/v1/servers/{id}/worlds/{wid}/backups-queue/{bid}/restore", post(backups::restore))
        .route("/v1/servers/{id}/worlds/{wid}/backups-queue/{bid}/retry", post(backups::retry))
        .route(
            "/v1/servers/{id}/worlds/{wid}/backups-queue/history/{kind}/{operation}/{action}",
            post(backups::history),
        )
        .route("/v1/servers/{id}/worlds/{wid}/select-download", get(backups::select_download))
        .route(
            "/servers/{id}/backups/schedule",
            get(backups::get_schedule).put(backups::set_schedule),
        )
        .route(
            "/servers/{id}/modrinth/v0/backups/{bid}/download/authorize",
            get(backups::authorize_backup),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/files/download-full-zip/authorize",
            get(backups::authorize_world),
        )
        .route(
            "/v1/servers/{id}/worlds/{wid}/properties",
            get(properties::get).patch(properties::patch),
        )
        .route("/v1/servers/{id}/worlds/{wid}/addons/delete", post(content::delete))
        .route("/v1/servers/{id}/worlds/{wid}/addons/delete-many", post(content::delete_many))
        .route("/v1/servers/{id}/worlds/{wid}/addons/disable", post(content::disable))
        .route("/v1/servers/{id}/worlds/{wid}/addons/enable", post(content::enable))
        .route("/v1/servers/{id}/worlds/{wid}/addons/disable-many", post(content::disable_many))
        .route("/v1/servers/{id}/worlds/{wid}/addons/enable-many", post(content::enable_many))
        .route("/v1/servers/{id}/worlds/{wid}/content", post(content::install))
        .route("/v1/servers/{id}/worlds/{wid}/content/repair", post(content::repair))
        .route("/v1/servers/{id}/worlds/{wid}/content/unlink-modpack", post(content::unlink))
        .route(
            "/v1/servers/{id}/worlds/{wid}/content/modpack/update",
            get(content::modpack).post(content::update_modpack),
        )
        .route("/servers/{id}/modrinth/v0/fs/list", get(files::list))
        .route("/servers/{id}/modrinth/v0/fs/download", get(files::download))
        .route("/servers/{id}/modrinth/v0/fs/move", post(files::rename))
        .route("/servers/{id}/modrinth/v0/fs/delete", delete(files::delete))
        .route("/servers/{id}/v1/fs/ops/{action}", post(files::operation))
        .route("/servers/{id}/v1/worlds/{wid}/files/stat", post(files::stat))
        .route(
            "/servers/{id}/v1/worlds/{wid}/{scope}/upload-session",
            get(files::get_session).post(files::open_session),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/{scope}/upload-session/{upload}",
            delete(files::cancel_session),
        )
        // Applies to the routes above only.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        // Starting a server answers once Java runs, and an install or update once it is done.
        .route("/modrinth/v0/servers/{id}/power", post(servers::power))
        .route("/update/apply", post(update::apply))
        .route("/servers/{id}/reachability", post(insight::check))
        .route(
            "/modrinth/v0/servers/{id}/reinstall",
            post(servers::reinstall),
        )
        // Adding and updating addons download them first, as does taking the pack's version of
        // an edited file.
        .route(
            "/servers/{id}/content/edited",
            get(content::edited).post(content::decide),
        )
        .route("/v1/servers/{id}/worlds/{wid}/addons", get(content::list).post(content::add))
        .route("/v1/servers/{id}/worlds/{wid}/addons/install-many", post(content::add_many))
        .route(
            "/v1/servers/{id}/worlds/{wid}/addons/update",
            get(content::get_update).post(content::update),
        )
        .route("/v1/servers/{id}/worlds/{wid}/addons/update-many", post(content::update_many))
        .route(
            "/servers/{id}/v1/worlds/{wid}/{scope}/upload-session/{upload}/finalize",
            post(files::finalize_session),
        )
        .route("/servers/{id}/v1/fs/unarchive", post(files::unarchive))
        .route("/servers/{id}/v1/worlds/{wid}/files/zip", post(files::zip))
        // Downloads stream for as long as they take.
        .route(
            "/servers/{id}/modrinth/v0/backups/{bid}/download",
            get(backups::download_backup),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/files/download-full-zip",
            get(backups::download_world),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY))
        // Uploads stream to disk with their own cap (`content::MAX_UPLOAD`).
        .route(
            "/servers/{id}/v1/worlds/{wid}/content/upload-modpack-file",
            post(content::upload_modpack),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/content/upload-addon-file",
            post(content::upload_addons),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/{scope}/upload-session/{upload}/files",
            post(files::upload_files),
        )
        .route("/servers/{id}/modrinth/v0/fs/create", post(files::create))
        .route("/servers/{id}/modrinth/v0/fs/update", put(files::update))
        .layer(DefaultBodyLimit::disable())
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

/// Serve HTTP/1 or HTTP/2, with upgrades for the console socket, on one connection. A connection
/// that sends no request for [`IDLE_TIMEOUT`] is closed.
pub async fn serve_connection<I>(io: I, router: Router) -> Result<()>
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut builder = Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(IDLE_TIMEOUT);
    builder
        .serve_connection_with_upgrades(
            TokioIo::new(io),
            TowerToHyperService::new(router),
        )
        .await
        .map_err(|error| eyre!("{error}"))
}
