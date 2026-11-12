mod dialog;
pub mod files;
mod network;
mod opener;
mod shortcuts;
mod thumbnails;
mod webview_log;

use std::{fmt::Display, sync::Arc};

use axum::Router;
use contracts::{ApiError, ErrorCode};

use crate::engine_handle::EngineHandle;

/// The Tauri app, which builds after the local server binds. Setup sets it; a route that needs it
/// waits until then.
pub type AppSlot = Arc<tokio::sync::SetOnce<tauri::AppHandle>>;

pub fn routes(engine: EngineHandle, app: AppSlot, secret: &str) -> Router {
    files::routes(engine.clone())
        .merge(dialog::routes(app))
        .merge(opener::routes(engine))
        .merge(shortcuts::routes())
        .merge(network::routes())
        .merge(webview_log::routes(secret))
}

fn internal(error: impl Display) -> ApiError {
    ApiError::new(ErrorCode::InternalError, error.to_string())
}
