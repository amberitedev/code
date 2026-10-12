use std::path::PathBuf;

use tracing::info;

use super::state::AppState;
use crate::infrastructure::minecraft::java::{
    download_java, installed_java_version, java_requirement, JavaInstall,
};

// A shared runtime is extracted once even when several servers start together.
static RUNTIME_INSTALL: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

pub async fn requirement(
    state: &AppState,
    game: &str,
    loader: Option<&str>,
) -> (u32, &'static str) {
    java_requirement(&state.http, &state.config.data_dir, game, loader).await
}

pub async fn ensure_java(
    state: &AppState,
    version: u32,
) -> Result<PathBuf, String> {
    let _guard = RUNTIME_INSTALL.lock().await;
    if let Some(path) = state.java_store.find_by_version(version).await {
        if installed_java_version(&path) == Some(version) {
            return Ok(path);
        }
    }
    let root = state.config.data_dir.join("java").join(version.to_string());
    let cached = tokio::task::spawn_blocking(move || {
        walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.file_name()
                        == if cfg!(windows) { "java.exe" } else { "java" }
            })
            .map(|entry| entry.into_path())
            .find(|path| installed_java_version(path) == Some(version))
    })
    .await
    .map_err(|e| e.to_string())?;
    let path = match cached {
        Some(path) => path,
        None => {
            download_java(&state.http, &state.config.data_dir, version).await?
        }
    };
    state
        .java_store
        .sync_all(&[JavaInstall {
            version,
            path: path.clone(),
        }])
        .await;
    info!(version, path = %path.display(), "Java runtime ready");
    Ok(path)
}
