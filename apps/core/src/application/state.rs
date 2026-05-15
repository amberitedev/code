use std::{
    path::PathBuf,
    sync::{atomic::AtomicU32, Arc},
    time::Instant,
};

use dashmap::DashMap;
use sqlx::SqlitePool;

use crate::{
    config::Config,
    domain::instance::InstanceId,
    infrastructure::{
        auth::jwks::JwksCache,
        db::{
            instance_repo::InstanceRepo, java_repo::JavaRepo,
            modpack_repo::ModpackRepo,
        },
        events::EventBroadcaster,
        macro_engine::executor::MacroExecutor,
        process::{instance_actor::InstanceHandle, pty_spawner::PtySpawner},
    },
    ports::{
        instance_store::InstanceStore, java_store::JavaStore,
        modpack_store::ModpackStore, process_spawner::AnySpawner,
    },
};

/// Short-lived ticket for WebSocket auth.
pub struct WsTicket {
    pub expires_at: Instant,
}

/// Short-lived token for one-time file downloads (issued by GET /instances/:id/fs/url).
pub struct FsDownloadToken {
    pub instance_id: String,
    pub path: PathBuf,
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
    /// Running instance handles, keyed by instance ID.
    pub instances: DashMap<InstanceId, InstanceHandle>,
    /// Broadcast channel for all instance events.
    pub broadcaster: EventBroadcaster,
    /// Deno macro executor.
    pub macro_executor: MacroExecutor,
    /// JWKS cache for Supabase JWT validation.
    pub jwks_cache: JwksCache,
    /// In-memory short-lived WebSocket tickets.
    pub ws_tickets: DashMap<String, WsTicket>,
    /// In-memory short-lived file download tokens (issued by GET /instances/:id/fs/url).
    pub fs_download_tokens: DashMap<String, FsDownloadToken>,
    /// First-run pairing code (cleared after pairing).
    pub pairing_code: tokio::sync::Mutex<Option<String>>,
    /// SEC-01: counts wrong pairing-code attempts; locked out after MAX_PAIRING_ATTEMPTS.
    pub wrong_pairing_attempts: AtomicU32,
    /// Instance data store.
    pub instance_store: Arc<dyn InstanceStore>,
    /// Java installation store.
    pub java_store: Arc<dyn JavaStore>,
    /// Modpack manifest store.
    pub modpack_store: Arc<dyn ModpackStore>,
    /// Process spawner — `PtySpawner` in production, `MockSpawner` in tests.
    pub spawner: Arc<dyn AnySpawner>,
}

impl AppState {
    /// Create a new `AppState` with production defaults (uses `PtySpawner`).
    pub async fn new(
        config: Config,
        pool: SqlitePool,
    ) -> color_eyre::eyre::Result<Arc<Self>> {
        Self::new_with_spawner(config, pool, Arc::new(PtySpawner)).await
    }

    /// Create a new `AppState` with a custom spawner (used in tests with `MockSpawner`).
    pub async fn new_with_spawner(
        config: Config,
        pool: SqlitePool,
        spawner: Arc<dyn AnySpawner>,
    ) -> color_eyre::eyre::Result<Arc<Self>> {
        let http = reqwest::Client::builder()
            .user_agent("amberite-core/0.1")
            .build()?;
        let broadcaster = EventBroadcaster::new();
        let jwks_cache = JwksCache::new(http.clone());

        let instance_store = Arc::new(InstanceRepo::new(pool.clone()));
        let java_store = Arc::new(JavaRepo::new(pool.clone()));
        let modpack_store = Arc::new(ModpackRepo::new(pool.clone()));

        // Generate first-run pairing code if not yet paired.
        let is_paired =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM core_config")
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
                > 0;

        // In dev mode, skip pairing entirely — no code, no banner.
        let pairing_code = if is_paired || config.dev_mode {
            None
        } else {
            let code = generate_pairing_code();
            println!("\n╔══════════════════════════════╗");
            println!("║  Amberite Core — Pairing Code  ║");
            println!("║          {code}          ║");
            println!("╚══════════════════════════════╝\n");
            Some(code)
        };

        Ok(Arc::new(Self {
            pool,
            http,
            config,
            instances: DashMap::new(),
            broadcaster,
            macro_executor: MacroExecutor::new(),
            jwks_cache,
            ws_tickets: DashMap::new(),
            fs_download_tokens: DashMap::new(),
            pairing_code: tokio::sync::Mutex::new(pairing_code),
            wrong_pairing_attempts: AtomicU32::new(0),
            instance_store,
            java_store,
            modpack_store,
            spawner,
        }))
    }

    /// JWKS URL derived from the stored supabase_url.
    pub async fn jwks_url(&self) -> Option<String> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT supabase_url FROM core_config WHERE id = 1")
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();
        row.map(|(url,)| format!("{url}/auth/v1/.well-known/jwks.json"))
    }

    /// Owner user id written during setup. Only this user may administer Core.
    pub async fn owner_user_id(&self) -> Option<String> {
        sqlx::query_scalar("SELECT owner_user_id FROM core_config WHERE id = 1")
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
    }
}

fn generate_pairing_code() -> String {
    use rand::Rng;
    let n: u32 = rand::thread_rng().gen_range(100_000..=999_999);
    n.to_string()
}
