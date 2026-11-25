//! The HTTP API: Axum routes, one file per resource. The same router serves HTTPS and the local
//! socket (see `access::local`).
//!
//! Routes only a Core has are `contracts::core` Endpoints, in Modrinth Hosting's shapes. A server's
//! content, share, files, icon and logs are the engine's routes (`engine_mount`).

pub mod activity;
pub mod backups;
pub mod console;
mod engine_mount;
pub mod eula;
pub mod events;
pub mod follow;
pub mod health;
pub mod insight;
pub mod logs;
pub mod pairing;
pub mod performance;
pub mod properties;
pub mod servers;
pub mod sessions;
pub mod startup;
pub mod storage;
#[cfg(test)]
mod tests;
pub mod update;

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{DefaultBodyLimit, FromRef},
    http::StatusCode,
    routing::any,
};
use color_eyre::eyre::{Result, eyre};
use contracts::{ApiError, ErrorCode, core::*, server::Mount};
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
    pub backups: Arc<crate::backups::Backups>,
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

impl FromRef<Shared> for Arc<Engine> {
    fn from_ref(shared: &Shared) -> Self {
        Arc::clone(&shared.0.engine)
    }
}

impl FromRef<Shared> for Arc<dyn Access> {
    fn from_ref(shared: &Shared) -> Self {
        Arc::clone(&shared.0.access) as Self
    }
}

/// Every route. Each one checks its caller itself (see `access`); the network listener adds its
/// own checks on top (see `network::tls`).
pub fn router(state: Arc<CoreState>) -> Router {
    Router::new()
        .endpoint::<CheckHealth, _, _>(health::get)
        .endpoint::<EventSocket, _, _>(events::socket)
        .endpoint::<SignIn, _, _>(sessions::sign_in)
        .endpoint::<ListSessions, _, _>(sessions::list)
        .endpoint::<SignOut, _, _>(sessions::sign_out)
        .endpoint::<RevokeSession, _, _>(sessions::revoke)
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
        .endpoint::<GetStartup, _, _>(startup::get)
        .endpoint::<ChangeStartup, _, _>(startup::change)
        .endpoint::<GetProperties, _, _>(properties::get)
        .endpoint::<ChangeProperties, _, _>(properties::change)
        .endpoint::<ListBackups, _, _>(backups::list)
        .endpoint::<CreateBackup, _, _>(backups::create)
        .endpoint::<ChangeBackupOptions, _, _>(backups::change_options)
        .endpoint::<DeleteBackups, _, _>(backups::delete_many)
        .endpoint::<RenameBackup, _, _>(backups::rename)
        .endpoint::<DeleteBackup, _, _>(backups::delete)
        .endpoint::<RestoreBackup, _, _>(backups::restore)
        .endpoint::<RetryBackup, _, _>(backups::retry)
        .endpoint::<AckOperation, _, _>(backups::ack)
        .endpoint::<CancelOperation, _, _>(backups::cancel)
        .endpoint::<SetJava, _, _>(startup::set_java_path)
        .merge(engine_mount::routes(Router::new()))
        // Applies to the routes above only.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        // Starting a server answers once Java runs, and an install or update once it is done.
        .endpoint::<ChangePower, _, _>(servers::power)
        .endpoint::<InstallUpdate, _, _>(update::apply)
        .endpoint::<CheckReachability, _, _>(insight::check)
        .endpoint::<InstallLoader, _, _>(servers::install)
        // A push uploads the server's files and waits for the store to confirm them.
        .endpoint::<ShareServer, _, _>(servers::share)
        // Following an instance reads its share's version first.
        .endpoint::<FollowInstance, _, _>(follow::follow)
        // Adding and updating addons download them first, as does taking the pack's version of
        // an edited file, and a pack's install or update backs the world up first. Reading,
        // unpacking, zipping, uploading and exporting files go through them whole.
        .merge(engine_mount::long_routes(Router::new()))
        // Downloads stream for as long as they take.
        .endpoint::<DownloadBackup, _, _>(backups::download_backup)
        .endpoint::<DownloadWorld, _, _>(backups::download_world)
        .endpoint::<ExportModpack, _, _>(engine_mount::export_mrpack)
        .layer(DefaultBodyLimit::max(MAX_BODY))
        // Uploads stream to disk with their own cap: the disk's free space (`archive::room`).
        .endpoint::<InstallModpackFile, _, _>(servers::install_modpack_file)
        .merge(engine_mount::upload_routes(Router::new()))
        // The pool's store streams uploads and downloads of any size, with its own caps. It is a
        // pass-through to the `copal store` process, which mounts its own Endpoints.
        .route("/store/{*rest}", any(storage::forward))
        .layer(DefaultBodyLimit::disable())
        .with_state(Shared(state))
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::extract::Request| {
                tracing::debug_span!(
                    "request",
                    method = %request.method(),
                    path = %request.uri().path(),
                )
            },
        ))
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
