use anyhow::Context;
use contracts::Side;
use engine::{AccountEndpoints, Engine, EngineConfig, EventSink};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::local_api::events::AppEvent;
use crate::settings::Settings;
use crate::state::{Desktop, DesktopState};

const LOGS_FOLDER: &str = "launcher_logs";

/// The folder holding app.db and the launcher logs: `AMBERITE_CONFIG_DIR`, else the OS data folder.
/// Instances and engine.db live here too unless settings name another data directory.
pub fn resolve(identifier: &str) -> anyhow::Result<PathBuf> {
    std::env::var_os("AMBERITE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| Some(dirs::data_dir()?.join(identifier)))
        .context("could not find the application directory")
}

pub fn logs_dir(identifier: &str) -> anyhow::Result<PathBuf> {
    Ok(resolve(identifier)?.join(LOGS_FOLDER))
}

/// Opens app.db, finishes a pending data-directory move and reads the limits, then opens the
/// engine on the data where it now is. A failed move keeps its `directory_move_*` code.
pub async fn open(
    settings_dir: PathBuf,
    events: EventSink,
    app_events: broadcast::Sender<AppEvent>,
    accounts: Option<AccountEndpoints>,
) -> anyhow::Result<Desktop> {
    let db = crate::db::open(&settings_dir).await?;
    let settings = Settings::get(&db).await?;
    let data_dir = engine::settings::prepare_data_dir(
        &settings_dir,
        settings.custom_dir.as_deref().map(Path::new),
        settings.prev_custom_dir.as_deref().map(Path::new),
        &events,
        async |dir: &Path| {
            let mut settings = Settings::get(&db).await?;
            let dir = dir.to_string_lossy().into_owned();
            settings.custom_dir = Some(dir.clone());
            settings.prev_custom_dir = Some(dir);
            settings.update(&db).await
        },
    )
    .await?;
    let state = Arc::new(DesktopState::new(db, app_events));
    let config = EngineConfig {
        instances_dir: data_dir.join(engine::INSTANCES_FOLDER_NAME),
        data_dir,
        logs_dir: Some(settings_dir.join(LOGS_FOLDER)),
        metadata_dir: None,
        side: Side::Client,
        max_concurrent_downloads: settings.max_concurrent_downloads,
        max_concurrent_writes: settings.max_concurrent_writes,
        events,
        accounts,
        hooks: Some(Arc::new(crate::hooks::DesktopHooks::new(Arc::clone(
            &state,
        )))),
        user_agent: crate::user_agent(),
    };
    Ok(Desktop::new(Engine::new(config).await?, state))
}

/// Drops a pending data-directory move, so the next start opens the previous directory.
pub async fn cancel_directory_change(identifier: &str) -> anyhow::Result<()> {
    let settings_dir = resolve(identifier)?;
    let db = crate::db::open(&settings_dir).await?;
    let mut settings = Settings::get(&db).await?;
    engine::settings::cancel_directory_change(
        &settings_dir,
        settings.prev_custom_dir.as_deref().map(Path::new),
    )
    .await?;
    if let Some(previous) = settings.prev_custom_dir.take() {
        settings.custom_dir = Some(previous);
    }
    settings.update(&db).await?;
    db.close().await;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An App on `dir` whose events nobody reads.
    pub(crate) async fn open(
        dir: &Path,
        accounts: Option<AccountEndpoints>,
    ) -> anyhow::Result<Desktop> {
        engine::install_crypto_provider();
        super::open(
            dir.to_path_buf(),
            EventSink::new(),
            broadcast::Sender::new(16),
            accounts,
        )
        .await
    }
}
