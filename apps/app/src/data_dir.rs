use anyhow::Context;
use contracts::Side;
use engine::{AccountEndpoints, EngineConfig, EventSink, data::Settings};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::settings::rows::SettingsRow;

const LOGS_FOLDER: &str = "launcher_logs";

/// The folder holding app.db and the launcher logs: `THESEUS_CONFIG_DIR`, else the OS data folder.
/// Instances and engine.db live here too unless settings name another data directory.
pub fn resolve(identifier: &str) -> anyhow::Result<PathBuf> {
    std::env::var_os("THESEUS_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| Some(dirs::data_dir()?.join(identifier)))
        .context("could not find the application directory")
}

pub fn logs_dir(identifier: &str) -> anyhow::Result<PathBuf> {
    Ok(resolve(identifier)?.join(LOGS_FOLDER))
}

/// Opens app.db, finishes a pending data-directory move and reads the limits, so the engine opens
/// on the data where it now is. A failed move keeps its `directory_move_*` code.
pub async fn engine_config(
    settings_dir: PathBuf,
    events: EventSink,
    accounts: Option<AccountEndpoints>,
) -> anyhow::Result<EngineConfig> {
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
    Ok(EngineConfig {
        instances_dir: data_dir.join(engine::INSTANCES_FOLDER_NAME),
        data_dir,
        logs_dir: Some(settings_dir.join(LOGS_FOLDER)),
        metadata_dir: None,
        side: Side::Client,
        max_concurrent_downloads: settings.max_concurrent_downloads,
        max_concurrent_writes: settings.max_concurrent_writes,
        events,
        accounts,
        hooks: Some(Arc::new(crate::hooks::DesktopHooks::new(db))),
    })
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
