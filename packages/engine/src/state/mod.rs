//! Theseus state management system
use crate::access::BoxFuture;
use crate::api::instance::GameOptionsPackSource;
use crate::event::EventSink;
use crate::util::fetch::{FetchSemaphore, IoSemaphore};
use contracts::Side;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::self_hosted::accounts::AccountEndpoints;
use crate::state::instances::watcher::FileWatcher;
use sqlx::SqlitePool;

// Submodules
mod dirs;
pub use self::dirs::*;

// instance_types.rs is compiled by bedrock along with the content model.
pub use bedrock::state::instance_types::*;

pub(crate) mod instances;
pub use self::instances::*;
pub(crate) use self::instances::{StoredOption, StoredPreference};
pub(crate) use self::instances::{
    game_options_sync_is_enabled, load_game_option_preferences,
    load_game_options_sync_state, load_shared_game_options,
    shared_game_options_exist,
};

mod settings;
pub use self::settings::*;

mod onboarding_checklist;
pub use self::onboarding_checklist::*;

mod process;
pub use self::process::*;

mod java_globals;
pub use self::java_globals::*;

mod cache;
pub use self::cache::*;

pub mod content_store;
pub(crate) mod runtime_cache;

mod installs;
pub(crate) use self::installs::Installs;

mod instance_locks;
pub(crate) use self::instance_locks::InstanceLocks;

mod sessions;
pub use self::sessions::*;

mod synced_options;
pub(crate) use self::synced_options::SyncedOptions;

pub(crate) mod accounts;
pub mod db;
pub(crate) mod db_backup;

pub mod attached_world_data;
pub mod hidden_instances;
pub mod server_join_log;

pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub instances_dir: PathBuf,
    pub logs_dir: Option<PathBuf>,
    /// app.db, which also holds the App's tables until they get their own database.
    pub db_path: PathBuf,
    pub side: Side,
    pub max_concurrent_downloads: usize,
    pub max_concurrent_writes: usize,
    pub events: EventSink,
    pub accounts: Option<AccountEndpoints>,
    pub pack_hooks: Option<Arc<dyn PackUpdateHooks>>,
}

/// Runs host work around a modpack update: `before` sees the instance's files as the user left
/// them, `after` the pack's.
pub trait PackUpdateHooks: Send + Sync {
    fn before_pack_update<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
    ) -> BoxFuture<'a, crate::Result<()>>;

    fn after_pack_update<'a>(
        &'a self,
        engine: &'a Engine,
        instance_id: &'a str,
        overrides: PackOverrides<'a>,
    ) -> BoxFuture<'a, crate::Result<()>>;
}

/// What a pack update wrote over the instance.
pub struct PackOverrides<'a> {
    pub servers: PackServers<'a>,
    pub game_options: Option<GameOptionsPackSource>,
}

pub enum PackServers<'a> {
    /// The update left the server list alone.
    Untouched,
    /// The instance ignores the pack's servers.
    Ignored,
    /// The pack's server list file, if it has one.
    Pack(Option<&'a str>),
}

pub struct Engine {
    pub side: Side,

    /// Information on the location of files used in the launcher
    pub directories: DirectoryInfo,
    pub content_store: content_store::ContentStore,

    /// Semaphore used to limit concurrent network requests and avoid errors
    pub fetch_semaphore: FetchSemaphore,
    /// Semaphore used to limit concurrent I/O and avoid errors
    pub io_semaphore: IoSemaphore,
    /// Semaphore to limit concurrent API requests. This is separate from the fetch semaphore
    /// to keep API functionality while the app is performing intensive tasks.
    pub api_semaphore: FetchSemaphore,
    pub(crate) installs: Arc<Installs>,
    pub(crate) instance_locks: InstanceLocks,
    pub(crate) synced_options: SyncedOptions,
    pub(crate) publish_reports: crate::api::instance::PublishReports,

    /// Process manager
    pub process_manager: ProcessManager,

    pub(crate) pool: SqlitePool,

    pub(crate) file_watcher: FileWatcher,

    pub events: EventSink,

    pub sessions: Sessions,

    pack_hooks: Option<Arc<dyn PackUpdateHooks>>,
}

impl Engine {
    /// Opens an engine on the folders `config` names, as they are: the host has finished any
    /// directory move first. Its background tasks hold a `Weak` and end once the last `Arc` is
    /// dropped.
    pub async fn new(config: EngineConfig) -> crate::Result<Arc<Self>> {
        let (state, watcher_events) = Self::open(config).await?;
        state.spawn_background(watcher_events);
        Ok(state)
    }

    /// The database, which also holds the App's account and skin tables.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// The fallible part of `new`; it spawns nothing.
    #[tracing::instrument(skip_all)]
    async fn open(
        config: EngineConfig,
    ) -> crate::Result<(
        Arc<Self>,
        tokio::sync::mpsc::Receiver<notify_debouncer_mini::DebounceEventResult>,
    )> {
        let EngineConfig {
            data_dir,
            instances_dir,
            logs_dir,
            db_path,
            side,
            max_concurrent_downloads,
            max_concurrent_writes,
            events,
            accounts,
            pack_hooks,
        } = config;
        let directories =
            DirectoryInfo::init(&db_path, data_dir, instances_dir, logs_dir)
                .await?;

        tracing::info!("Connecting to app database");
        let store_lock = content_store::ContentStore::lock_process(
            &directories.settings_dir,
        )
        .await?;
        let pool = db::open(&db_path).await?;

        let fetch_semaphore =
            FetchSemaphore(Arc::new(Semaphore::new(max_concurrent_downloads)));
        let io_semaphore = IoSemaphore(Semaphore::new(max_concurrent_writes));
        let api_semaphore =
            FetchSemaphore(Arc::new(Semaphore::new(max_concurrent_downloads)));

        let content_store = content_store::ContentStore::new(
            &directories,
            pool.clone(),
            store_lock,
        )
        .await?;

        tracing::info!("Initializing file watcher");
        let (file_watcher, watcher_events) =
            instances::watcher::init_watcher()?;

        let state = Arc::new(Self {
            side,
            directories,
            content_store,
            fetch_semaphore,
            io_semaphore,
            api_semaphore,
            installs: Arc::new(Installs::new()),
            instance_locks: InstanceLocks::default(),
            synced_options: SyncedOptions::default(),
            publish_reports: Default::default(),
            process_manager: ProcessManager::new(),
            pool,
            file_watcher,
            events,
            sessions: Sessions::new(accounts),
            pack_hooks,
        });

        state.content_store.recover(None).await?;
        crate::install::recovery::recover_interrupted_jobs(&state).await?;
        state
            .content_store
            .recover_abandoned(&state.directories.install_backups_dir())
            .await?;
        Ok((state, watcher_events))
    }

    fn spawn_background(
        self: &Arc<Self>,
        watcher_events: tokio::sync::mpsc::Receiver<
            notify_debouncer_mini::DebounceEventResult,
        >,
    ) {
        instances::watcher::watch_instance_events(
            Arc::downgrade(self),
            watcher_events,
        );

        let maintained = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(600));
            interval.set_missed_tick_behavior(
                tokio::time::MissedTickBehavior::Skip,
            );
            loop {
                interval.tick().await;
                let Some(state) = maintained.upgrade() else {
                    return;
                };
                if let Err(error) =
                    CachedEntry::purge_long_expired(&state.pool).await
                {
                    tracing::warn!("Expired cache cleanup deferred: {error}");
                }
                if let Err(error) =
                    state.content_store.cleanup(&state, false).await
                {
                    tracing::debug!(
                        "Shared content cache cleanup deferred: {error}"
                    );
                }
            }
        });

        let starting = Arc::downgrade(self);
        tokio::spawn(async move {
            let Some(state) = starting.upgrade() else {
                return;
            };
            instances::watcher::watch_instances_init(
                &state.file_watcher,
                &state.directories,
                &state.pool,
            )
            .await;
            if let Err(error) = refresh_all_instances(&state).await {
                tracing::error!("Failed to refresh instances: {error}");
            }
            if let Err(error) = Settings::migrate(&state.pool).await {
                tracing::error!("Failed to migrate settings: {error}");
            }
        });
    }

    pub(crate) async fn before_pack_update(
        &self,
        instance_id: &str,
    ) -> crate::Result<()> {
        match &self.pack_hooks {
            Some(hooks) => hooks.before_pack_update(self, instance_id).await,
            None => Ok(()),
        }
    }

    pub(crate) async fn after_pack_update(
        &self,
        instance_id: &str,
        overrides: PackOverrides<'_>,
    ) -> crate::Result<()> {
        match &self.pack_hooks {
            Some(hooks) => {
                hooks.after_pack_update(self, instance_id, overrides).await
            }
            None => Ok(()),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::state::instances::adapters::sqlite::{
        content_rows, instance_rows,
    };
    use std::path::Path;

    pub(crate) async fn add_instance(state: &Engine, id: &str, path: &str) {
        let now = chrono::Utc::now();
        let content_set_id = format!("content-set:{id}");
        let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        instance_rows::insert_instance(
            &Instance {
                id: id.to_string(),
                path: path.to_string(),
                applied_content_set_id: Some(content_set_id.clone()),
                install_stage: InstanceInstallStage::Installed,
                launcher_feature_version: LauncherFeatureVersion::MOST_RECENT,
                update_channel: ReleaseChannel::Release,
                name: id.to_string(),
                icon_path: None,
                created: now,
                modified: now,
                last_played: None,
                submitted_time_played: 0,
                recent_time_played: 0,
            },
            &mut tx,
        )
        .await
        .unwrap();
        content_rows::insert_content_set(
            &ContentSet {
                id: content_set_id,
                instance_id: id.to_string(),
                name: "Default".to_string(),
                source_kind: ContentSourceKind::Local,
                status: ContentSetStatus::Available,
                game_version: "1.21.1".to_string(),
                protocol_version: None,
                loader: ModLoader::Fabric,
                loader_version: None,
                created: now,
                modified: now,
            },
            &mut tx,
        )
        .await
        .unwrap();
        instance_rows::upsert_instance_link(
            id,
            &InstanceLink::Unmanaged,
            &mut tx,
        )
        .await
        .unwrap();
        instance_rows::upsert_instance_launch_overrides(
            &InstanceLaunchOverrides::empty(id.to_string()),
            &mut tx,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    /// An App engine keeping everything in `dir`.
    pub(crate) fn config(dir: PathBuf) -> EngineConfig {
        EngineConfig {
            instances_dir: dir.join(INSTANCES_FOLDER_NAME),
            db_path: dir.join("app.db"),
            data_dir: dir,
            logs_dir: None,
            side: Side::Client,
            max_concurrent_downloads: 10,
            max_concurrent_writes: 10,
            events: EventSink::new(),
            accounts: None,
            pack_hooks: None,
        }
    }

    pub(crate) async fn engine(dir: PathBuf) -> Arc<Engine> {
        Engine::new(config(dir)).await.unwrap()
    }

    pub(super) fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, path.to_string_lossy().as_bytes()).unwrap();
    }

    async fn content_paths(state: &Engine, id: &str) -> Vec<String> {
        let mut paths: Vec<_> = sync_content_files(id, state)
            .await
            .unwrap()
            .into_iter()
            .map(|file| file.relative_path)
            .collect();
        paths.sort();
        paths
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn two_engines_list_their_own_content() {
        crate::install_crypto_provider();
        let tmp = tempfile::tempdir().unwrap();
        let (a_dir, b_dir) = (tmp.path().join("a"), tmp.path().join("b"));
        write(&a_dir.join("profiles/Survival/mods/a.jar"));
        write(&b_dir.join("profiles/Survival/mods/b.jar"));
        write(&b_dir.join("profiles/Survival/resourcepacks/b.zip"));

        let (a, b) = (engine(a_dir).await, engine(b_dir).await);
        add_instance(&a, "a", "Survival").await;
        add_instance(&b, "b", "Survival").await;

        let (a_paths, b_paths) =
            tokio::join!(content_paths(&a, "a"), content_paths(&b, "b"));
        assert_eq!(a_paths, ["mods/a.jar"]);
        assert_eq!(b_paths, ["mods/b.jar", "resourcepacks/b.zip"]);

        let listed = crate::api::instance::list(&b).await.unwrap();
        let ids: Vec<_> = listed
            .iter()
            .map(|metadata| &metadata.instance.id)
            .collect();
        assert_eq!(ids, ["b"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn removal_waits_for_a_synced_options_pass_in_flight() {
        crate::install_crypto_provider();
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("profiles/Survival/options.txt"));
        let state = engine(tmp.path().to_path_buf()).await;
        add_instance(&state, "a", "Survival").await;
        let folder = state.directories.instances_dir().join("Survival");

        let pass = state.synced_options.lock().await;
        let removal = tokio::spawn({
            let state = Arc::clone(&state);
            async move { remove_instance("a", &state).await }
        });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(folder.exists() && !removal.is_finished());
        drop(pass);
        removal.await.unwrap().unwrap();
        assert!(!folder.exists());
    }
}
