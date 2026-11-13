mod debug_export;
mod dialog;
mod external_save;
pub mod files;
mod jre;
mod network;
mod opener;
mod opening;
mod shortcuts;
mod thumbnails;
pub mod updater;
mod upload;
mod webview_log;

use std::{fmt::Display, sync::Arc};

use axum::Router;
use contracts::{ApiError, ErrorCode};

use crate::{engine_handle::EngineHandle, server::AppState};

/// The Tauri app, which builds after the local server binds. Setup sets it; a route that needs it
/// waits until then.
pub type AppSlot = Arc<tokio::sync::SetOnce<tauri::AppHandle>>;

pub fn routes(engine: EngineHandle, app: AppSlot, secret: &str) -> Router {
    files::routes(engine.clone())
        .merge(dialog::routes(app.clone()))
        .merge(opening::routes(app))
        .merge(opener::routes(engine))
        .merge(shortcuts::routes())
        .merge(network::routes())
        .merge(jre::routes())
        .merge(webview_log::routes(secret))
}

/// The long operations, which `crate::server::ops` runs.
pub fn op_routes() -> Router<AppState> {
    debug_export::routes()
        .merge(external_save::routes())
        .merge(upload::routes())
}

pub(crate) fn internal(error: impl Display) -> ApiError {
    ApiError::new(ErrorCode::InternalError, error.to_string())
}
