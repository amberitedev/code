use std::{
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use dashmap::DashMap;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    config::Config,
    domain::instance::InstanceId,
    infrastructure::{
        db::{
            installation_repo::InstallationRepo, instance_repo::InstanceRepo,
            java_repo::JavaRepo, modpack_repo::ModpackRepo,
        },
        events::EventBroadcaster,
        process::{instance_actor::InstanceHandle, std_spawner::StdSpawner},
    },
    presentation::extractors::Account,
    ports::{
        installation_store::InstallationStore, instance_store::InstanceStore,
        java_store::JavaStore, modpack_store::ModpackStore,
        process_spawner::AnySpawner,
    },
};

/// Short-lived ticket for WebSocket auth.
pub struct WsTicket {
    pub user_id: String,
    pub expires_at: Instant,
}

/// Short-lived token for one-time file downloads (issued by GET /instances/:id/fs/url).
pub struct FsDownloadToken {
    pub path: PathBuf,
    pub expires_at: Instant,
}

/// In-progress resumable upload tracked by Core.
#[derive(Clone)]
pub struct FsUploadSession {
    pub instance_id: String,
    pub destination: PathBuf,
    pub partial_path: PathBuf,
    pub length: u64,
    pub offset: u64,
    pub sha256: Option<String>,
    pub expires_at: Instant,
}

/// Central shared state — passed as `Arc<AppState>` through all layers.
pub struct AppState {
    /// SQLite connection pool (kept for legacy/direct queries).
    pub pool: SqlitePool,
    /// Shared HTTP client.
    pub http: reqwest::Client,
    /// Runtime config.
    pub config: Config,
    /// Stable Core identity generated once and persisted locally.
    pub core_id: String,
    /// Running instance handles, keyed by instance ID.
    pub instances: DashMap<InstanceId, InstanceHandle>,
    /// Short-lived per-instance operation locks for lifecycle mutations.
    pub instance_operation_locks:
        DashMap<InstanceId, Arc<tokio::sync::Mutex<()>>>,
    /// Broadcast channel for all instance events.
    pub broadcaster: EventBroadcaster,
    /// Verified account tokens: token -> (account, expiry). See `AuthUser`.
    pub account_tokens: DashMap<String, (Account, Instant)>,
    /// In-memory short-lived WebSocket tickets.
    pub ws_tickets: DashMap<String, WsTicket>,
    /// In-memory short-lived file download tokens (issued by GET /instances/:id/fs/url).
    pub fs_download_tokens: DashMap<String, FsDownloadToken>,
    /// In-memory resumable upload sessions.
    pub fs_upload_sessions: DashMap<String, FsUploadSession>,
    /// Instance data store.
    pub instance_store: Arc<dyn InstanceStore>,
    /// Shared server installation store.
    pub installation_store: Arc<dyn InstallationStore>,
    /// Java installation store.
    pub java_store: Arc<dyn JavaStore>,
    /// Modpack manifest store.
    pub modpack_store: Arc<dyn ModpackStore>,
    /// Process spawner — `StdSpawner` in production, `MockSpawner` in tests.
    pub spawner: Arc<dyn AnySpawner>,
}

impl AppState {
    /// Create a new `AppState` with production defaults (uses `StdSpawner`).
    pub async fn new(
        config: Config,
        pool: SqlitePool,
    ) -> color_eyre::eyre::Result<Arc<Self>> {
        Self::new_with_spawner(config, pool, Arc::new(StdSpawner)).await
    }

    /// Create a new `AppState` with a custom spawner (used in tests with `MockSpawner`).
    pub async fn new_with_spawner(
        config: Config,
        pool: SqlitePool,
        spawner: Arc<dyn AnySpawner>,
    ) -> color_eyre::eyre::Result<Arc<Self>> {
        let http =
            reqwest::Client::builder().user_agent("copal/0.1").build()?;
        let broadcaster = EventBroadcaster::new();

        let instance_store = Arc::new(InstanceRepo::new(pool.clone()));
        let installation_store = Arc::new(InstallationRepo::new(pool.clone()));
        let java_store = Arc::new(JavaRepo::new(pool.clone()));
        let modpack_store = Arc::new(ModpackRepo::new(pool.clone()));
        let core_id = load_or_create_core_id(&pool).await?;

        Ok(Arc::new(Self {
            pool,
            http,
            config,
            core_id,
            instances: DashMap::new(),
            instance_operation_locks: DashMap::new(),
            broadcaster,
            account_tokens: DashMap::new(),
            ws_tickets: DashMap::new(),
            fs_download_tokens: DashMap::new(),
            fs_upload_sessions: DashMap::new(),
            instance_store,
            installation_store,
            java_store,
            modpack_store,
            spawner,
        }))
    }

    /// The account that owns this Core, if one has connected yet.
    pub async fn owner_user_id(&self) -> Option<String> {
        sqlx::query_scalar("SELECT user_id FROM core_owner WHERE id = 1")
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
    }

    /// Make `user_id` the owner if this Core has none, then return the owner.
    pub async fn claim_owner(&self, user_id: &str) -> sqlx::Result<String> {
        sqlx::query(
            "INSERT OR IGNORE INTO core_owner (id, user_id) VALUES (1, ?)",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        sqlx::query_scalar("SELECT user_id FROM core_owner WHERE id = 1")
            .fetch_one(&self.pool)
            .await
    }
}

async fn load_or_create_core_id(
    pool: &SqlitePool,
) -> color_eyre::eyre::Result<String> {
    if let Some(core_id) = sqlx::query_scalar::<_, String>(
        "SELECT core_id FROM core_identity WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok(core_id);
    }

    let core_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT OR IGNORE INTO core_identity (id, core_id, created_at) VALUES (1, ?, ?)",
    )
    .bind(&core_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;

    Ok(sqlx::query_scalar::<_, String>(
        "SELECT core_id FROM core_identity WHERE id = 1",
    )
    .fetch_one(pool)
    .await?)
}
