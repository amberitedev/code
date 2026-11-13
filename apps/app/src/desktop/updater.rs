//! Serves `crate::server::endpoints::updates`. Only a build with the `updater` feature registers
//! Tauri's updater plugin, so only it can find an update; the App installs a downloaded one on exit.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use axum::{Router, extract::State, http::StatusCode};
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    server::{Req, route},
};
use engine::{
    Engine, LoadingBarType, emit_loading, init_loading, launcher_user_agent,
    routes::api_error,
};
use tauri::http::{HeaderValue, header::ACCEPT};
use tauri_plugin_http::reqwest::{self, ClientBuilder};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::time::Instant;

use super::{AppSlot, internal};
use crate::server::{
    AppState,
    endpoints::updates::{
        AvailableUpdate, CheckForUpdate, DiscardDownloadedUpdate,
        DownloadUpdate, GetUpdateSize, RestartApp, SetRestartAfterUpdate,
    },
    ops::Ops,
};

pub struct Updater {
    app: AppSlot,
    /// What the last check found.
    available: Mutex<Option<Arc<Update>>>,
    /// Installed when the App exits.
    downloaded: Mutex<Option<(Arc<Update>, Vec<u8>)>>,
    restart_after: AtomicBool,
}

impl Updater {
    pub fn new(app: AppSlot) -> Self {
        Self {
            app,
            available: Mutex::default(),
            downloaded: Mutex::default(),
            restart_after: AtomicBool::default(),
        }
    }

    #[cfg(feature = "updater")]
    pub fn take_downloaded(&self) -> Option<(Arc<Update>, Vec<u8>)> {
        self.downloaded.lock().unwrap().take()
    }

    #[cfg(any(test, feature = "updater"))]
    pub fn restarts_after(&self) -> bool {
        self.restart_after.load(Ordering::Relaxed)
    }

    fn available(&self) -> Result<Arc<Update>, ApiError> {
        self.available.lock().unwrap().clone().ok_or_else(|| {
            ApiError::new(ErrorCode::NotFound, "No update has been found")
        })
    }
}

pub fn routes() -> Router<AppState> {
    let (check_path, check) = route::<CheckForUpdate, _, _, _>(check);
    let (size_path, size) = route::<GetUpdateSize, _, _, _>(size);
    let (download_path, download) = route::<DownloadUpdate, _, _, _>(download);
    let (discard_path, discard) =
        route::<DiscardDownloadedUpdate, _, _, _>(discard);
    let (restart_after_path, restart_after) =
        route::<SetRestartAfterUpdate, _, _, _>(set_restart_after);
    let (restart_path, restart) = route::<RestartApp, _, _, _>(restart);
    Router::new()
        .route(check_path, check)
        .route(size_path, size)
        .route(download_path, download)
        .route(discard_path, discard)
        .route(restart_after_path, restart_after)
        .route(restart_path, restart)
}

async fn check(
    State(updater): State<Arc<Updater>>,
    _: Req<CheckForUpdate>,
) -> Result<Json<Option<AvailableUpdate>>, ApiError> {
    if !cfg!(feature = "updater") {
        return Err(ApiError::new(
            ErrorCode::Unsupported,
            "Updates are disabled in this build",
        ));
    }
    let found = updater
        .app
        .wait()
        .await
        .updater()
        .map_err(internal)?
        .check()
        .await
        .map_err(internal)?
        .map(Arc::new);
    let available = found.as_ref().map(|update| AvailableUpdate {
        version: update.version.clone(),
    });
    *updater.available.lock().unwrap() = found;
    Ok(Json(available))
}

/// Asks for the headers alone, as `Update::download` would ask for the file.
async fn size(
    State(updater): State<Arc<Updater>>,
    _: Req<GetUpdateSize>,
) -> Result<Json<Option<u64>>, ApiError> {
    let update = updater.available()?;
    let mut headers = update.headers.clone();
    if !headers.contains_key(ACCEPT) {
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/octet-stream"),
        );
    }
    let mut request = ClientBuilder::new().user_agent(launcher_user_agent());
    if let Some(timeout) = update.timeout {
        request = request.timeout(timeout);
    }
    if let Some(proxy) = &update.proxy {
        request = request
            .proxy(reqwest::Proxy::all(proxy.as_str()).map_err(internal)?);
    }
    let response = request
        .build()
        .map_err(internal)?
        .head(update.download_url.clone())
        .headers(headers)
        .send()
        .await
        .map_err(internal)?;
    if !response.status().is_success() {
        return Err(internal(format!(
            "Download request failed with status: {}",
            response.status()
        )));
    }
    Ok(Json(
        response
            .headers()
            .get("Content-Length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok()),
    ))
}

async fn download(
    State(engine): State<Arc<Engine>>,
    State(ops): State<Arc<Ops>>,
    State(updater): State<Arc<Updater>>,
    Req { body, .. }: Req<DownloadUpdate>,
) -> Result<StatusCode, ApiError> {
    let update = updater.available()?;
    ops.start(body.op, |op| async move {
        let progress = init_loading(
            &engine.events,
            LoadingBarType::LauncherUpdate {
                version: update.version.clone(),
                current_version: update.current_version.clone(),
            },
            1.0,
            "Downloading update...",
        )
        .await
        .map_err(api_error)?;
        let started = Instant::now();
        let bytes = op
            .until_cancelled(async {
                update
                    .download(
                        |chunk, total| {
                            let Some(total) = total else { return };
                            if let Err(e) = emit_loading(
                                &progress,
                                chunk as f64 / total as f64,
                                None,
                            ) {
                                tracing::error!(
                                    "Failed to update download progress bar: {e}"
                                );
                            }
                        },
                        || {},
                    )
                    .await
                    .map_err(internal)
            })
            .await?;
        tracing::info!("Downloaded update in {:?}", started.elapsed());
        *updater.downloaded.lock().unwrap() = Some((update, bytes));
        Ok(None)
    })
}

async fn discard(
    State(updater): State<Arc<Updater>>,
    _: Req<DiscardDownloadedUpdate>,
) -> Empty {
    updater.downloaded.lock().unwrap().take();
    Empty
}

async fn set_restart_after(
    State(updater): State<Arc<Updater>>,
    Req { body, .. }: Req<SetRestartAfterUpdate>,
) -> Empty {
    updater.restart_after.store(body.restart, Ordering::Relaxed);
    Empty
}

async fn restart(
    State(updater): State<Arc<Updater>>,
    _: Req<RestartApp>,
) -> Empty {
    updater.app.wait().await.restart()
}
