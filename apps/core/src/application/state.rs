use std::{sync::Arc, time::Instant};

use dashmap::DashMap;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    config::Config,
    domain::instance::InstanceId,
    infrastructure::{
        db::{
            installation_repo::InstallationRepo, instance_repo::InstanceRepo,
            java_repo::JavaRepo,
        },
        events::EventBroadcaster,
        process::{instance_actor::InstanceHandle, std_spawner::StdSpawner},
    },
    network::{tls::Identity, Network},
    ports::{
        installation_store::InstallationStore, instance_store::InstanceStore,
        java_store::JavaStore, process_spawner::AnySpawner,
    },
    presentation::extractors::Account,
};

/// Short-lived, single-use ticket that opens one server's WebSocket.
pub struct WsTicket {
    pub instance_id: InstanceId,
    /// Who asked for the ticket. Commands sent on the socket are checked and logged as them.
    pub account: Account,
    pub expires_at: Instant,
}

/// The account this Core is signed in to.
pub struct Owner {
    pub user_id: String,
    /// Key for this Core's own calls to the backend, issued when the owner approved the sign-in.
    pub backend_key: String,
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
    /// Wakes the sign-in task when the owner changes or the backend refused the key. See
    /// `auth_service`.
    pub auth_changed: tokio::sync::Notify,
    /// In-memory short-lived WebSocket tickets.
    pub ws_tickets: DashMap<String, WsTicket>,
    /// Instance data store.
    pub instance_store: Arc<dyn InstanceStore>,
    /// Shared server installation store.
    pub installation_store: Arc<dyn InstallationStore>,
    /// Java installation store.
    pub java_store: Arc<dyn JavaStore>,
    /// Process spawner — `StdSpawner` in production, `MockSpawner` in tests.
    pub spawner: Arc<dyn AnySpawner>,
    /// The certificate Core serves its API with. The App pins its fingerprint.
    pub tls: Identity,
    /// What is known about reaching this Core and its servers from the internet.
    pub network: Network,
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
        let core_id = load_or_create_core_id(&pool).await?;
        let tls = Identity::load_or_create(&config.data_dir)?;

        Ok(Arc::new(Self {
            pool,
            http,
            config,
            core_id,
            instances: DashMap::new(),
            instance_operation_locks: DashMap::new(),
            broadcaster,
            account_tokens: DashMap::new(),
            auth_changed: tokio::sync::Notify::new(),
            ws_tickets: DashMap::new(),
            instance_store,
            installation_store,
            java_store,
            spawner,
            tls,
            network: Network::default(),
        }))
    }

    /// The account this Core is signed in to, if any. An owner row without a key is from before
    /// Cores had keys and counts as signed out: the next sign-in replaces it.
    pub async fn owner(&self) -> sqlx::Result<Option<Owner>> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT user_id, backend_key FROM core_owner WHERE id = 1 AND backend_key IS NOT NULL",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(user_id, backend_key)| Owner {
            user_id,
            backend_key,
        }))
    }

    /// Store the owner and the key that was approved for this Core.
    pub async fn set_owner(
        &self,
        user_id: &str,
        key: &str,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO core_owner (id, user_id, backend_key) VALUES (1, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET user_id = excluded.user_id, backend_key = excluded.backend_key",
        )
        .bind(user_id)
        .bind(key)
        .execute(&self.pool)
        .await?;
        self.auth_changed.notify_one();
        // The outside test needs the key that was just stored.
        self.network.wake();
        Ok(())
    }

    /// Forget the owner and the backend key. Servers and files stay. The caller wakes the
    /// sign-in task if it is not the one calling.
    pub async fn forget_owner(&self) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM core_owner")
            .execute(&self.pool)
            .await?;
        // The next owner starts with nobody else on these servers.
        super::access_service::forget_everyone(self).await?;
        self.account_tokens.clear();
        Ok(())
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
