//! Local Hosting compatibility routes over the existing Core services.
pub mod events;
pub mod files;
pub mod http;
pub mod models;

use crate::application::state::AppState;
use axum::Router;
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(http::router())
        .merge(files::router())
        .merge(events::router())
}
