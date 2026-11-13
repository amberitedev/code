//! Theseus state management system
use crate::access::BoxFuture;
use crate::api::server_list::{self, DesyncServerMode, ServerRecord};
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

pub mod instances;
pub use self::instances::*;

mod settings;
pub use self::settings::*;

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

pub mod db;
pub(crate) mod db_backup;

pub mod attached_world_data;
pub mod server_join_log;

pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub instances_dir: PathBuf,
    pub logs_dir: Option<PathBuf>,
    pub side: Side,
    pub max_concurrent_downloads: usize,
    pub max_concurrent_writes: usize,
    pub events: EventSink,
    pub accounts: Option<AccountEndpoints>,
    pub hooks: Option<Arc<dyn InstanceHooks>>,
}

/// Host work the engine's own instance changes wait on, the host's view of an instance's server
/// list, and the saved secrets logs hide. The App keeps synced options, packs and servers here;
/// the defaults are an engine with nothing synced or saved, which edits `servers.dat` directly.
pub trait InstanceHooks: std::any::Any + Send + Sync {
    fn secrets<'a>(
        &'a self,
        _engine: &'a Engine,
    ) -> BoxFuture<'a, crate::Result<SavedSecrets>> {
        Box::pin(async { Ok(SavedSecrets::default()) })
    }

    /// Runs before a modpack update, while the instance's files are as the user left them.
    fn before_pack_update<'a>(
        &'a self,
        _engine: &'a Engine,
        _instance_id: &'a str,
    ) -> BoxFuture<'a, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    /// Runs after a modpack update, over the pack's files.
    fn after_pack_update<'a>(
        &'a self,
        _engine: &'a Engine,
        _instance_id: &'a str,
        _overrides: PackOverrides<'a>,
    ) -> BoxFuture<'a, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    /// Held while an instance's folder is deleted.
    fn removing<'a>(
        &'a self,
        _engine: &'a Engine,
    ) -> BoxFuture<'a, Box<dyn Send + 'a>> {
        Box::pin(async { Box::new(()) as Box<dyn Send> })
    }

    fn decorate_content<'a>(
        &'a self,
        _engine: &'a Engine,
        _instance_id: &'a str,
        _items: &'a mut [ContentItem],
    ) -> BoxFuture<'a, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    /// A user installed `path` into the instance.
    fn content_installed<'a>(
        &'a self,
        _engine: &'a Engine,
        _instance_id: &'a str,
        _path: &'a str,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }

    /// A user updated, toggled or removed the content at `paths`.
    fn content_changed<'a>(
        &'a self,
        _engine: &'a Engine,
        _instance_id: &'a str,
        _paths: &'a [&'a str],
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }

    fn list_servers<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
    ) -> BoxFuture<'a, crate::Result<Vec<ServerRecord>>> {
        Box::pin(async move {
            server_list::list_local(&engine.servers_file(metadata)).await
        })
    }

    /// Adds a server and returns its id.
    fn add_server<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
        data: quartz_nbt::NbtCompound,
    ) -> BoxFuture<'a, crate::Result<String>> {
        Box::pin(async move {
            server_list::add_local(&engine.servers_file(metadata), data)
                .await?;
            Ok(uuid::Uuid::new_v4().to_string())
        })
    }

    /// Adds a server a linked server project needs, unless one at its address is listed.
    fn ensure_managed_server<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
        data: quartz_nbt::NbtCompound,
    ) -> BoxFuture<'a, crate::Result<()>> {
        Box::pin(async move {
            let path = engine.servers_file(metadata);
            let address = server_list::server_identity_address(&data);
            let listed =
                server_list::read_servers(&path)
                    .await?
                    .iter()
                    .any(|server| {
                        server_list::server_identity_address(server) == address
                    });
            if listed {
                return Ok(());
            }
            server_list::add_local(&path, data).await
        })
    }

    fn update_server<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
        index: usize,
        name: String,
        address: String,
        accept_textures: Option<bool>,
    ) -> BoxFuture<'a, crate::Result<()>> {
        Box::pin(async move {
            server_list::update_local(
                &engine.servers_file(metadata),
                index,
                name,
                address,
                accept_textures,
            )
            .await
        })
    }

    fn remove_server<'a>(
        &'a self,
        engine: &'a Engine,
        metadata: &'a InstanceMetadata,
        index: usize,
    ) -> BoxFuture<'a, crate::Result<()>> {
        Box::pin(async move {
            server_list::remove_local(&engine.servers_file(metadata), index)
                .await
        })
    }

    /// Removes the servers `ensure_managed_server` added.
    fn remove_managed_servers<'a>(
        &'a self,
        _engine: &'a Engine,
        _instance_id: &'a str,
    ) -> BoxFuture<'a, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    /// Drops the servers a modpack supplied.
    fn discard_modpack_servers<'a>(
        &'a self,
        _engine: &'a Engine,
        _instance_id: &'a str,
    ) -> BoxFuture<'a, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn desync_server<'a>(
        &'a self,
        _engine: &'a Engine,
        _instance_id: &'a str,
        _server_id: &'a str,
        _mode: DesyncServerMode,
    ) -> BoxFuture<'a, crate::Result<()>> {
        Box::pin(async {
            Err(crate::ErrorKind::InputError(
                "server syncing is not available".to_string(),
            )
            .into())
        })
    }
}

struct NoHooks;

/// The host's saved accounts, which game logs and support details hide.
#[derive(Default)]
pub struct SavedSecrets {
    pub minecraft: Vec<SavedMinecraftAccount>,
    pub modrinth_sessions: Vec<String>,
}

pub struct SavedMinecraftAccount {
    pub access_token: String,
    pub name: String,
    pub id: uuid::Uuid,
}

impl InstanceHooks for NoHooks {}

/// What a pack update wrote over the instance.
pub struct PackOverrides<'a> {
    pub servers: PackServers<'a>,
    pub game_options: Option<GameOptionsPackSource>,
}

/// Where a modpack keeps the `options.txt` it ships.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameOptionsPackSource {
    ClientOverrides,
    Overrides,
    ClientOverridesYosbr,
    OverridesYosbr,
    ClientOverridesDefaultOptions,
    OverridesDefaultOptions,
}

impl GameOptionsPackSource {
    pub const YOSBR_OPTIONS_PATH: &str = "config/yosbr/options.txt";
    pub const DEFAULT_OPTIONS_PATH: &str = "config/defaultoptions/options.txt";

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClientOverrides
            | Self::ClientOverridesYosbr
            | Self::ClientOverridesDefaultOptions => "client_overrides",
            Self::Overrides
            | Self::OverridesYosbr
            | Self::OverridesDefaultOptions => "overrides",
        }
    }

    pub fn template_path(self) -> Option<&'static str> {
        match self {
            Self::ClientOverridesYosbr | Self::OverridesYosbr => {
                Some(Self::YOSBR_OPTIONS_PATH)
            }
            Self::ClientOverridesDefaultOptions
            | Self::OverridesDefaultOptions => Some(Self::DEFAULT_OPTIONS_PATH),
            Self::ClientOverrides | Self::Overrides => None,
        }
    }
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
    pub(crate) publish_reports: crate::api::instance::PublishReports,

    /// Process manager
    pub process_manager: ProcessManager,

    pub(crate) pool: SqlitePool,

    pub(crate) file_watcher: FileWatcher,

    pub events: EventSink,

    pub sessions: Sessions,

    hooks: Arc<dyn InstanceHooks>,
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

    /// engine.db.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn hooks(&self) -> &dyn InstanceHooks {
        &*self.hooks
    }

    fn servers_file(&self, metadata: &InstanceMetadata) -> PathBuf {
        self.directories
            .instances_dir()
            .join(&metadata.instance.path)
            .join(server_list::SERVERS_FILE)
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
            side,
            max_concurrent_downloads,
            max_concurrent_writes,
            events,
            accounts,
            hooks,
        } = config;
        let directories =
            DirectoryInfo::init(data_dir, instances_dir, logs_dir).await?;

        tracing::info!("Connecting to the engine database");
        let store_lock =
            content_store::ContentStore::lock_process(&directories.config_dir)
                .await?;
        let pool = db::open(&directories.config_dir).await?;

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
            publish_reports: Default::default(),
            process_manager: ProcessManager::new(),
            pool,
            file_watcher,
            events,
            sessions: Sessions::new(accounts),
            hooks: hooks.unwrap_or_else(|| Arc::new(NoHooks)),
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
        });
    }

    pub(crate) async fn before_pack_update(
        &self,
        instance_id: &str,
    ) -> crate::Result<()> {
        self.hooks.before_pack_update(self, instance_id).await
    }

    pub(crate) async fn after_pack_update(
        &self,
        instance_id: &str,
        overrides: PackOverrides<'_>,
    ) -> crate::Result<()> {
        self.hooks
            .after_pack_update(self, instance_id, overrides)
            .await
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
            data_dir: dir,
            logs_dir: None,
            side: Side::Client,
            max_concurrent_downloads: 10,
            max_concurrent_writes: 10,
            events: EventSink::new(),
            accounts: None,
            hooks: None,
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
}
