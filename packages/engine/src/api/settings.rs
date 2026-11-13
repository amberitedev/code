//! Theseus settings management interface

use crate::event::EventSink;
use crate::state::content_store::ContentStore;
pub use crate::state::content_store::StoreUsage;
use crate::state::{DirectoryInfo, db};
use crate::util::fetch::IoSemaphore;
pub use crate::{
    Engine,
    state::{Hooks, MemorySettings, Settings, WindowSize},
};
use std::path::Path;
use tokio::sync::Semaphore;

pub async fn store_usage(state: &Engine) -> crate::Result<StoreUsage> {
    state.content_store.usage(state).await
}

pub async fn store_cleanup(state: &Engine) -> crate::Result<u64> {
    state.content_store.cleanup(state, true).await
}

pub async fn store_set_cache_limit(
    state: &Engine,
    bytes: u64,
) -> crate::Result<()> {
    state.content_store.set_cache_limit(bytes).await
}

/// Backs up and opens the database at `db_path`, records this version, and finishes a pending
/// data-directory move, whose default destination is `settings_dir`. The App runs it before
/// `Engine::new`; the returned settings name the data directory and the limits.
#[tracing::instrument(skip(events))]
pub async fn prepare_data_dir(
    db_path: &Path,
    settings_dir: &Path,
    events: &EventSink,
) -> crate::Result<Settings> {
    let _lock = ContentStore::lock_process(settings_dir).await?;
    let pool = db::connect_app_db(db_path).await?;
    db::record_current_app_version(&pool).await?;
    let mut settings = Settings::get(&pool).await?;
    let io_semaphore =
        IoSemaphore(Semaphore::new(settings.max_concurrent_writes));
    DirectoryInfo::move_launcher_directory(
        &mut settings,
        &pool,
        &io_semaphore,
        settings_dir,
        events,
    )
    .await?;
    pool.close().await;
    Ok(settings)
}

#[tracing::instrument]
pub async fn cancel_directory_change(db_path: &Path) -> crate::Result<()> {
    // This is called to handle state initialization errors due to folder migrations
    // failing, so there may be no engine to take a DB connection pool from
    let pool = db::open(db_path).await?;
    let mut settings = Settings::get(&pool).await?;
    crate::state::move_app_directory::cancel_move(&pool).await?;

    if let Some(prev_custom_dir) = settings.prev_custom_dir {
        settings.prev_custom_dir = None;
        settings.custom_dir = Some(prev_custom_dir);
    }

    settings.update(&pool).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn preparing_finishes_a_pending_directory_move() {
        let tmp = tempfile::tempdir().unwrap();
        let settings_dir = tmp.path().join("settings");
        let (old, new) = (tmp.path().join("old"), tmp.path().join("new"));
        let db_path = settings_dir.join("app.db");
        let options = std::path::Path::new("profiles/Survival/options.txt");
        std::fs::create_dir_all(old.join(options).parent().unwrap()).unwrap();
        std::fs::write(old.join(options), "fov:90").unwrap();
        let pool = db::open(&db_path).await.unwrap();
        let mut settings = Settings::get(&pool).await.unwrap();
        settings.prev_custom_dir = Some(old.to_string_lossy().into_owned());
        settings.custom_dir = Some(new.to_string_lossy().into_owned());
        settings.update(&pool).await.unwrap();
        pool.close().await;

        let settings =
            prepare_data_dir(&db_path, &settings_dir, &EventSink::new())
                .await
                .unwrap();

        assert_eq!(settings.custom_dir, settings.prev_custom_dir);
        assert_eq!(
            settings.custom_dir,
            Some(new.to_string_lossy().into_owned())
        );
        assert_eq!(
            std::fs::read_to_string(new.join(options)).unwrap(),
            "fov:90"
        );
        assert!(!old.join("profiles").exists());
    }
}
