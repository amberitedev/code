//! The HTTP API: Axum routes, one file per resource. The same router serves HTTPS and the local
//! socket (see `access::local`).
//!
//! Routes only a Core has are `contracts::core` Endpoints. The content, share, file, icon and
//! download routes match the `Archon.*` and `Kyros.*` shapes in `packages/api-client/src/modules`;
//! the share, diff, history and edited-file types among them generate their TypeScript here (see
//! `typescript`).

pub mod access;
pub mod activity;
pub mod backups;
pub mod console;
pub mod content;
pub mod eula;
pub mod events;
pub mod export;
pub mod files;
pub mod health;
mod icon;
pub mod insight;
pub mod logs;
pub mod pairing;
pub mod performance;
pub mod properties;
pub mod servers;
pub mod sessions;
pub mod share;
pub mod startup;
pub mod storage;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod typescript;
pub mod update;

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{DefaultBodyLimit, FromRef},
    handler::Handler,
    http::StatusCode,
    routing::{any, delete, get, post, put},
};
use color_eyre::eyre::{Result, eyre};
use contracts::{ApiError, Endpoint, ErrorCode, core::*, server::route};
use engine::{Engine, access::Access};
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use sqlx::SqlitePool;
use tokio::io::{AsyncRead, AsyncWrite};
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

/// A failure on the Core's side, such as a database or disk error, described by its cause.
pub fn internal(error: impl std::fmt::Display) -> ApiError {
    ApiError::new(ErrorCode::InternalError, format!("{error:#}"))
}

/// The longest a request may take, except the routes that wait on a server.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a connection may sit without a request, or take to send one's headers.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Bodies are capped at this; upload routes stream to disk with their own cap.
const MAX_BODY: usize = 10 * 1024 * 1024;

/// What every route can reach.
pub struct CoreState {
    pub db: SqlitePool,
    pub engine: Arc<Engine>,
    pub servers: Arc<crate::servers::Servers>,
    pub access: Arc<crate::access::CoreAccess>,
    pub content: Arc<crate::state::servers::Content>,
    pub backups: Arc<crate::backups::Backups>,
    pub uploads: files::Uploads,
    pub update: Arc<crate::update::Updater>,
    pub network: Arc<crate::network::forward::Network>,
    pub storage: Arc<crate::storage::Storage>,
    pub consoles: Arc<events::Consoles>,
}

/// The router's state. Routes take `State<Arc<CoreState>>`; the engine's extractors find
/// [`Access`] through it.
#[derive(Clone)]
pub struct Shared(pub Arc<CoreState>);

impl FromRef<Shared> for Arc<CoreState> {
    fn from_ref(shared: &Shared) -> Self {
        Arc::clone(&shared.0)
    }
}

impl FromRef<Shared> for Arc<dyn Access> {
    fn from_ref(shared: &Shared) -> Self {
        Arc::clone(&shared.0.access) as Self
    }
}

/// Mounting an Endpoint's handler at its method and path.
trait Endpoints {
    fn endpoint<E: Endpoint, H: Handler<T, Shared>, T: 'static>(
        self,
        handler: H,
    ) -> Self;
}

impl Endpoints for Router<Shared> {
    fn endpoint<E: Endpoint, H: Handler<T, Shared>, T: 'static>(
        self,
        handler: H,
    ) -> Self {
        let (path, method) = route::<E, _, _, _>(handler);
        self.route(path, method)
    }
}

/// Every route. Each one checks its caller itself (see `access`); the network listener adds its
/// own checks on top (see `network::tls`).
pub fn router(state: Arc<CoreState>) -> Router {
    Router::new()
        .endpoint::<CheckHealth, _, _>(health::get)
        .route("/events", get(events::socket))
        .endpoint::<SignIn, _, _>(sessions::sign_in)
        .endpoint::<ListSessions, _, _>(sessions::list)
        .endpoint::<SignOut, _, _>(sessions::sign_out)
        .endpoint::<RevokeSession, _, _>(sessions::revoke)
        .endpoint::<GetMyAccess, _, _>(access::me)
        .endpoint::<ListMembers, _, _>(access::list)
        .endpoint::<InviteMember, _, _>(access::invite)
        .endpoint::<ChangeRole, _, _>(access::change_role)
        .endpoint::<RemoveMember, _, _>(access::remove)
        .endpoint::<ListActivity, _, _>(activity::list)
        .endpoint::<GetPairing, _, _>(pairing::get)
        .endpoint::<RequestPairingCode, _, _>(pairing::new_code)
        .endpoint::<Unpair, _, _>(pairing::remove)
        .endpoint::<GetUpdate, _, _>(update::get)
        .endpoint::<ChangeUpdateSettings, _, _>(update::settings)
        .endpoint::<CheckForUpdate, _, _>(update::check)
        .endpoint::<RollBack, _, _>(update::rollback)
        .endpoint::<GetStorage, _, _>(storage::get)
        .endpoint::<ChangeStorage, _, _>(storage::settings)
        .endpoint::<GetEula, _, _>(eula::get)
        .endpoint::<AcceptEula, _, _>(eula::post)
        .endpoint::<GetLogs, _, _>(logs::get)
        .endpoint::<GetPerformance, _, _>(performance::get)
        .endpoint::<SetMemory, _, _>(performance::set_memory)
        .endpoint::<SetRestarts, _, _>(performance::set_restarts)
        .endpoint::<SendCommand, _, _>(console::send)
        .endpoint::<SetFlags, _, _>(performance::set_flags)
        .endpoint::<GetInsight, _, _>(insight::get)
        .endpoint::<GetMachine, _, _>(insight::machine)
        .endpoint::<ListServers, _, _>(servers::list)
        .endpoint::<CreateServer, _, _>(servers::create)
        .endpoint::<GetServer, _, _>(servers::get)
        .endpoint::<ChangeServer, _, _>(servers::change)
        .endpoint::<DeleteServer, _, _>(servers::delete)
        .endpoint::<GetSetup, _, _>(servers::setup)
        .endpoint::<SetJavaVersion, _, _>(startup::set_java_version)
        .endpoint::<GetProperties, _, _>(properties::get)
        .endpoint::<ChangeProperties, _, _>(properties::change)
        .endpoint::<ListBackups, _, _>(backups::list)
        .endpoint::<CreateBackup, _, _>(backups::create)
        .endpoint::<ChangeBackupSchedule, _, _>(backups::change_schedule)
        .endpoint::<DeleteBackups, _, _>(backups::delete_many)
        .endpoint::<RenameBackup, _, _>(backups::rename)
        .endpoint::<DeleteBackup, _, _>(backups::delete)
        .endpoint::<RestoreBackup, _, _>(backups::restore)
        .endpoint::<RetryBackup, _, _>(backups::retry)
        .endpoint::<AckOperation, _, _>(backups::ack)
        .endpoint::<CancelOperation, _, _>(backups::cancel)
        .route(
            "/v1/servers/{id}/icon",
            get(icon::get).put(icon::set).delete(icon::delete),
        )
        .route("/v1/servers/{id}/icon/metadata", get(icon::metadata))
        .route("/v1/servers/{id}/icon/generated", put(icon::set_generated))
        .route("/modrinth/v0/servers/{id}/fs", get(files::auth))
        .endpoint::<SetJava, _, _>(startup::set_java_path)
        .route(
            "/servers/{id}/v1/worlds/{wid}/backups/{bid}/download/authorize",
            get(backups::authorize_backup),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/files/download-full-zip/authorize",
            get(backups::authorize_world),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/files/contents-raw/authorize",
            get(files::authorize_file),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/content/export-mrpack/authorize",
            get(export::authorize),
        )
        .route("/v1/servers/{id}/worlds/{wid}/addons/delete", post(content::delete))
        .route("/v1/servers/{id}/worlds/{wid}/addons/delete-many", post(content::delete_many))
        .route("/v1/servers/{id}/worlds/{wid}/addons/disable", post(content::disable))
        .route("/v1/servers/{id}/worlds/{wid}/addons/enable", post(content::enable))
        .route("/v1/servers/{id}/worlds/{wid}/addons/disable-many", post(content::disable_many))
        .route("/v1/servers/{id}/worlds/{wid}/addons/enable-many", post(content::enable_many))
        .route(
            "/v1/servers/{id}/worlds/{wid}/addons/set-enabled-server",
            post(content::set_enabled_server),
        )
        .route(
            "/v1/servers/{id}/worlds/{wid}/addons/set-enabled-player",
            post(content::set_enabled_player),
        )
        .route(
            "/v1/servers/{id}/worlds/{wid}/addons/set-locked-side-toggle",
            post(content::set_side_toggle_locked),
        )
        .route(
            "/v1/servers/{id}/worlds/{wid}/addons/complete-side-type-review",
            post(content::complete_side_type_review),
        )
        .route("/v1/servers/{id}/worlds/{wid}/content/share/diff", get(share::diff))
        .route("/v1/servers/{id}/worlds/{wid}/content/share/files", get(share::files))
        .route("/v1/servers/{id}/worlds/{wid}/content/share/history", get(share::history))
        .route(
            "/v1/servers/{id}/worlds/{wid}/content/share/history/{version}",
            delete(share::delete_version),
        )
        .route("/v1/servers/{id}/worlds/{wid}/content", post(content::install))
        .route("/v1/servers/{id}/worlds/{wid}/content/repair", post(content::repair))
        .route("/v1/servers/{id}/worlds/{wid}/content/unlink-modpack", post(content::unlink))
        .route(
            "/v1/servers/{id}/worlds/{wid}/content/modpack/update",
            get(content::modpack).post(content::update_modpack),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/content/embedded-icon",
            get(content::embedded_icon),
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
        .endpoint::<ChangePower, _, _>(servers::power)
        .endpoint::<InstallUpdate, _, _>(update::apply)
        // A push uploads the server's files and waits for the store to confirm them.
        .route(
            "/v1/servers/{id}/worlds/{wid}/content/share",
            post(share::share).delete(share::unshare),
        )
        // A restore downloads the version's Modrinth files and writes them before it answers.
        .route(
            "/v1/servers/{id}/worlds/{wid}/content/share/history/{version}/restore",
            post(share::restore),
        )
        .endpoint::<CheckReachability, _, _>(insight::check)
        .endpoint::<InstallLoader, _, _>(servers::install)
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
            "/servers/{id}/v1/worlds/{wid}/backups/{bid}/download",
            get(backups::download_backup),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/files/download-full-zip",
            get(backups::download_world),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/files/contents-raw",
            get(files::download_file),
        )
        .route(
            "/servers/{id}/v1/worlds/{wid}/content/export-mrpack",
            get(export::export),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY))
        // Uploads stream to disk with their own cap: the disk's free space (`archive::room`).
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
        // The pool's store streams uploads and downloads of any size, with its own caps.
        .route("/store/{*rest}", any(storage::forward))
        .layer(DefaultBodyLimit::disable())
        .with_state(Shared(state))
        .layer(TraceLayer::new_for_http())
}

/// Serve HTTP/1 or HTTP/2, with upgrades for the `/events` socket, on one connection. A connection
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
