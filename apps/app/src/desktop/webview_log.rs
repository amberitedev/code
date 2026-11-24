use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    Empty,
    server::{Mount, Req},
};

use crate::local_api::endpoints::desktop::{LogFromWebview, WebviewLogLevel};

/// Forwards the page's console to `LogFromWebview`.
pub fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("logs")
        .js_init_script(include_str!("webview-logs.js"))
        .build()
}

/// The page's console prints local URLs, and the log is shared, so the secret is masked.
pub fn routes(secret: &str) -> Router {
    Router::new()
        .endpoint::<LogFromWebview, _, _>(log)
        .with_state(Arc::<str>::from(secret))
}

async fn log(
    State(secret): State<Arc<str>>,
    Req { body, .. }: Req<LogFromWebview>,
) -> Empty {
    for line in body {
        let message = line.message.replace(&*secret, "<secret>");
        match line.level {
            WebviewLogLevel::Trace => {
                tracing::trace!(target: "webview", "{message}")
            }
            WebviewLogLevel::Debug => {
                tracing::debug!(target: "webview", "{message}")
            }
            WebviewLogLevel::Info => {
                tracing::info!(target: "webview", "{message}")
            }
            WebviewLogLevel::Warn => {
                tracing::warn!(target: "webview", "{message}")
            }
            WebviewLogLevel::Error => {
                tracing::error!(target: "webview", "{message}")
            }
        }
    }
    Empty
}
