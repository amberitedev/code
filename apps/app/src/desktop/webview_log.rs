use axum::Router;
use contracts::{
    Empty,
    server::{Req, route},
};

use crate::server::endpoints::desktop::{LogFromWebview, WebviewLogLevel};

pub fn routes() -> Router {
    let (path, handler) = route::<LogFromWebview, _, _, _>(log);
    Router::new().route(path, handler)
}

async fn log(Req { body, .. }: Req<LogFromWebview>) -> Empty {
    let message = body.message;
    match body.level {
        WebviewLogLevel::Trace => {
            tracing::trace!(target: "webview", "{message}")
        }
        WebviewLogLevel::Debug => {
            tracing::debug!(target: "webview", "{message}")
        }
        WebviewLogLevel::Info => tracing::info!(target: "webview", "{message}"),
        WebviewLogLevel::Warn => tracing::warn!(target: "webview", "{message}"),
        WebviewLogLevel::Error => {
            tracing::error!(target: "webview", "{message}")
        }
    }
    Empty
}
