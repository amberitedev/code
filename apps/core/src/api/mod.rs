//! The HTTP API: Axum routes, one file per resource, each with typed request and response structs.
//!
//! Hosting routes are served under `/hosting` and match the `Archon.*` shapes in
//! `packages/api-client/src/modules/archon/types.ts`, which the App already sends. Routes only
//! Amberite has (access, sessions, activity, backend sync) generate their TypeScript from these
//! structs with `ts-rs`; see `typescript`.

mod access;
mod activity;
mod backend;
mod error;
mod health;
mod sessions;
#[cfg(test)]
mod typescript;

use std::sync::Arc;

use axum::{Router, routing::get};
use sqlx::SqlitePool;
use tower_http::trace::TraceLayer;

/// What every route can reach.
pub struct AppState {
    pub db: SqlitePool,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health::get))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}
