//! Serves the link, path and folder routes of `crate::server::endpoints::desktop`.

use std::path::PathBuf;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, ErrorCode,
    server::{Req, route},
};
use engine::{instance::ScreenshotKey, prelude::app_db_backup_dir};
use url::Url;

use super::internal;
use crate::{
    engine_handle::EngineHandle,
    server::endpoints::desktop::{
        AppFolder, OpenFolder, OpenPath, OpenUrl, RevealPath,
    },
};

pub fn routes(engine: EngineHandle) -> Router {
    let (url_at, url_route) = route::<OpenUrl, _, _, _>(open_url);
    let (path_at, path_route) = route::<OpenPath, _, _, _>(open_path);
    let (reveal_at, reveal_route) = route::<RevealPath, _, _, _>(reveal_path);
    let (folder_at, folder_route) = route::<OpenFolder, _, _, _>(open_folder);
    Router::new()
        .route(url_at, url_route)
        .route(path_at, path_route)
        .route(reveal_at, reveal_route)
        .route(folder_at, folder_route)
        .with_state(engine)
}

async fn open_url(Req { body, .. }: Req<OpenUrl>) -> Result<Empty, ApiError> {
    let url = web_link(&body.url)?;
    run(move || tauri_plugin_opener::open_url(url.as_str(), None::<&str>)).await
}

/// What the native window's opener allowed: anything else could start a program by its scheme.
fn web_link(url: &str) -> Result<Url, ApiError> {
    Url::parse(url)
        .ok()
        .filter(|url| {
            matches!(url.scheme(), "http" | "https" | "mailto" | "tel")
        })
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::InvalidInput,
                "Only web, mail and phone links open.",
            )
        })
}

async fn open_path(Req { body, .. }: Req<OpenPath>) -> Result<Empty, ApiError> {
    open(body.path).await
}

async fn reveal_path(
    Req { body, .. }: Req<RevealPath>,
) -> Result<Empty, ApiError> {
    reveal(body.path).await
}

async fn open_folder(
    State(engine): State<EngineHandle>,
    Req { body, .. }: Req<OpenFolder>,
) -> Result<Empty, ApiError> {
    match body {
        AppFolder::LauncherLogs => {
            let path = engine
                .get()
                .await
                .directories
                .launcher_logs_dir()
                .ok_or_else(|| internal("the engine keeps no launcher logs"))?;
            open(path).await
        }
        AppFolder::AppDbBackups => {
            let path = app_db_backup_dir().map_err(internal)?;
            tokio::fs::create_dir_all(&path).await.map_err(internal)?;
            open(path).await
        }
        AppFolder::SyncedOptions => {
            let state = engine.get().await;
            open(
                engine::instance::get_synced_options_folder(&state)
                    .await
                    .map_err(internal)?,
            )
            .await
        }
        AppFolder::Screenshot {
            instance_id,
            file_name,
        } => {
            let state = engine.get().await;
            let key = ScreenshotKey {
                instance_id,
                file_name,
            };
            reveal(
                engine::instance::get_screenshot_path(&state, &key)
                    .await
                    .map_err(internal)?,
            )
            .await
        }
    }
}

async fn open(path: PathBuf) -> Result<Empty, ApiError> {
    run(move || tauri_plugin_opener::open_path(path, None::<&str>)).await
}

async fn reveal(path: PathBuf) -> Result<Empty, ApiError> {
    run(move || tauri_plugin_opener::reveal_item_in_dir(path)).await
}

async fn run(
    action: impl FnOnce() -> Result<(), tauri_plugin_opener::Error> + Send + 'static,
) -> Result<Empty, ApiError> {
    tokio::task::spawn_blocking(action)
        .await
        .map_err(internal)?
        .map_err(internal)?;
    Ok(Empty)
}

#[cfg(test)]
mod tests {
    use super::web_link;

    #[test]
    fn only_web_mail_and_phone_links_open() {
        let rows = [
            ("https://modrinth.com/mods", true),
            ("http://example.com", true),
            ("mailto:support@modrinth.com", true),
            ("tel:+15555550100", true),
            ("file:///C:/Windows/System32/calc.exe", false),
            ("ms-settings:privacy", false),
            ("javascript:alert(1)", false),
            ("not a url", false),
        ];
        for (url, opens) in rows {
            assert_eq!(web_link(url).is_ok(), opens, "{url}");
        }
    }
}
