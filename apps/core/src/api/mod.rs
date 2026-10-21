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
mod content;
pub mod error;
pub mod eula;
mod events;
mod files;
mod health;
pub mod logs;
pub mod performance;
mod properties;
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
/// Bodies are capped at this; upload routes, when they land, stream to disk with their own cap.
const MAX_BODY: usize = 10 * 1024 * 1024;

/// What every route can reach.
pub struct AppState {
    pub db: SqlitePool,
    pub servers: Arc<crate::servers::Servers>,
    pub access: Arc<crate::access::Access>,
    pub update: Arc<crate::update::Updater>,
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
        .route("/update", get(update::get).patch(update::settings))
        .route("/update/check", post(update::check))
        .route("/update/rollback", post(update::rollback))
        .route("/eula", get(eula::get).post(eula::post))
        .route("/logs", get(logs::get))
        .route("/servers/{id}/performance", get(performance::get))
        .route("/servers/{id}/memory", put(performance::set_memory))
        .route("/servers/{id}/restarts", put(performance::set_restarts))
        .route("/servers/{id}/console", post(console::send))
        .route("/modrinth/v0/servers", get(servers::list))
        .route("/modrinth/v0/servers/{id}", get(servers::get))
        .route("/modrinth/v0/servers/{id}/ws", get(servers::ws_auth))
        .route("/servers/{id}/ws", get(events::console))
        .route("/local/servers", post(servers::create))
        .route("/local/servers/{id}", delete(servers::delete))
        .route("/local/servers/{id}/eula", post(servers::accept_eula))
        // Applies to the routes above only.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        // Starting a server answers once Java runs, and an install or update once it is done.
        .route("/modrinth/v0/servers/{id}/power", post(servers::power))
        .route("/update/apply", post(update::apply))
        .route(
            "/modrinth/v0/servers/{id}/reinstall",
            post(servers::reinstall),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY))
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
