use anyhow::Context;
use contracts::Side;
use engine::{AccountEndpoints, EngineConfig, EventSink};
use std::path::PathBuf;

const DB_FILE: &str = "app.db";
const LOGS_FOLDER: &str = "launcher_logs";

/// The folder holding app.db and the launcher logs: `THESEUS_CONFIG_DIR`, else the OS data folder.
/// Instances live here too unless settings name another data directory.
pub fn resolve(identifier: &str) -> anyhow::Result<PathBuf> {
    std::env::var_os("THESEUS_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| Some(dirs::data_dir()?.join(identifier)))
        .context("could not find the application directory")
}

pub fn logs_dir(identifier: &str) -> anyhow::Result<PathBuf> {
    Ok(resolve(identifier)?.join(LOGS_FOLDER))
}

/// Backs up the database, finishes a pending data-directory move and reads the limits, so the
/// engine opens on the data where it now is. A failed move keeps its `directory_move_*` code.
pub async fn engine_config(
    settings_dir: PathBuf,
    events: EventSink,
    accounts: Option<AccountEndpoints>,
) -> anyhow::Result<EngineConfig> {
    let db_path = settings_dir.join(DB_FILE);
    let settings =
        engine::settings::prepare_data_dir(&db_path, &settings_dir, &events)
            .await?;
    let data_dir = settings
        .custom_dir
        .map_or_else(|| settings_dir.clone(), PathBuf::from);
    Ok(EngineConfig {
        instances_dir: data_dir.join(engine::INSTANCES_FOLDER_NAME),
        data_dir,
        logs_dir: Some(settings_dir.join(LOGS_FOLDER)),
        db_path,
        side: Side::Client,
        max_concurrent_downloads: settings.max_concurrent_downloads,
        max_concurrent_writes: settings.max_concurrent_writes,
        events,
        accounts,
    })
}

/// Drops a pending data-directory move, so the next start opens the previous directory.
pub async fn cancel_directory_change(identifier: &str) -> anyhow::Result<()> {
    engine::settings::cancel_directory_change(
        &resolve(identifier)?.join(DB_FILE),
    )
    .await
}
