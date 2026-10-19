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
mod backups;
mod content;
pub mod error;
mod events;
mod files;
mod health;
mod properties;
pub mod servers;
pub mod sessions;
mod startup;
#[cfg(test)]
mod typescript;

use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post},
};
use color_eyre::eyre::{Result, eyre};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use sqlx::SqlitePool;
use tokio::io::{AsyncRead, AsyncWrite};
use tower_http::trace::TraceLayer;

/// What every route can reach.
pub struct AppState {
    pub db: SqlitePool,
    pub servers: Arc<crate::servers::Servers>,
    pub access: Arc<crate::access::Access>,
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
        .route("/modrinth/v0/servers", get(servers::list))
        .route("/modrinth/v0/servers/{id}", get(servers::get))
        .route("/modrinth/v0/servers/{id}/ws", get(servers::ws_auth))
        .route("/servers/{id}/ws", get(events::console))
        .route("/local/servers", post(servers::create))
        .route("/local/servers/{id}", delete(servers::delete))
        .route("/local/servers/{id}/eula", post(servers::accept_eula))
        .route("/modrinth/v0/servers/{id}/power", post(servers::power))
        .route(
            "/modrinth/v0/servers/{id}/reinstall",
            post(servers::reinstall),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

/// Serve HTTP/1 or HTTP/2, with upgrades for the console socket, on one connection.
pub async fn serve_connection<I>(io: I, router: Router) -> Result<()>
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    Builder::new(TokioExecutor::new())
        .serve_connection_with_upgrades(
            TokioIo::new(io),
            TowerToHyperService::new(router),
        )
        .await
        .map_err(|error| eyre!("{error}"))
}
