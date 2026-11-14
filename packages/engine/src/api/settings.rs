//! Theseus settings management interface

use crate::event::EventSink;
use crate::state::content_store::ContentStore;
pub use crate::state::content_store::StoreUsage;
use crate::state::{DirectoryInfo, db};
pub use crate::{
    Engine,
    state::{Hooks, MemorySettings, Settings, WindowSize},
};
use std::path::{Path, PathBuf};

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

/// Finishes a pending data-directory move before `Engine::new`, carrying engine.db, and returns
/// the data directory: `custom_dir`, else `settings_dir`. `commit` records it in the host's
/// settings as both the custom and the previous directory.
#[tracing::instrument(skip(events, commit))]
pub async fn prepare_data_dir(
    settings_dir: &Path,
    custom_dir: Option<&Path>,
    prev_custom_dir: Option<&Path>,
    events: &EventSink,
    commit: impl AsyncFnOnce(&Path) -> crate::Result<()>,
) -> crate::Result<PathBuf> {
    let _lock = ContentStore::lock_process(settings_dir).await?;
    let data_dir = custom_dir.unwrap_or(settings_dir);
    DirectoryInfo::move_launcher_directory(
        prev_custom_dir,
        data_dir,
        settings_dir,
        events,
        commit,
    )
    .await?;
    Ok(data_dir.to_path_buf())
}

/// Drops a pending data-directory move, so the next start opens the previous directory again.
#[tracing::instrument]
pub async fn cancel_directory_change(
    settings_dir: &Path,
    prev_custom_dir: Option<&Path>,
) -> crate::Result<()> {
    // This is called to handle state initialization errors due to folder migrations
    // failing, so there may be no engine to take a DB connection pool from
    let pool = db::open(prev_custom_dir.unwrap_or(settings_dir)).await?;
    crate::state::move_app_directory::cancel_move(&pool, prev_custom_dir)
        .await?;
    pool.close().await;
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
        let options = std::path::Path::new("profiles/Survival/options.txt");
        std::fs::create_dir_all(old.join(options).parent().unwrap()).unwrap();
        std::fs::write(old.join(options), "fov:90").unwrap();
        let pool = db::open(&old).await.unwrap();
        crate::state::content_store::set_setting(&pool, "moved", "yes")
            .await
            .unwrap();
        pool.close().await;

        let mut committed = None;
        let data_dir = prepare_data_dir(
            &settings_dir,
            Some(&new),
            Some(&old),
            &EventSink::new(),
            async |dir: &Path| {
                committed = Some(dir.to_path_buf());
                Ok(())
            },
        )
        .await
        .unwrap();

        assert_eq!(data_dir, new);
        assert_eq!(committed, Some(new.clone()));
        assert_eq!(
            std::fs::read_to_string(new.join(options)).unwrap(),
            "fov:90"
        );
        assert!(!old.join("profiles").exists());
        assert!(!old.join(db::DB_FILE).exists());
        let pool = db::open(&new).await.unwrap();
        assert_eq!(
            crate::state::content_store::setting(&pool, "moved")
                .await
                .unwrap()
                .as_deref(),
            Some("yes")
        );
    }
}
