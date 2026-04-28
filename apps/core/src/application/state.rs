use std::{sync::Arc, time::Instant};

use dashmap::DashMap;
use sqlx::SqlitePool;

use crate::{
    config::Config,
    domain::instance::InstanceId,
    infrastructure::{
        auth::jwks::JwksCache,
        db::{instance_repo::InstanceRepo, modpack_repo::ModpackRepo},
        events::EventBroadcaster,
        macro_engine::executor::MacroExecutor,
        process::instance_actor::InstanceHandle,
    },
    ports::{instance_store::InstanceStore, modpack_store::ModpackStore},
};

/// Short-lived ticket for WebSocket auth.
pub struct WsTicket {
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
    /// First-run pairing code (cleared after pairing).
    pub pairing_code: tokio::sync::Mutex<Option<String>>,
    /// Instance data store.
    pub instance_store: Arc<dyn InstanceStore>,
    /// Modpack manifest store.
    pub modpack_store: Arc<dyn ModpackStore>,
}

impl AppState {
    pub async fn new(config: Config, pool: SqlitePool) -> color_eyre::eyre::Result<Arc<Self>> {
        let http = reqwest::Client::builder()
            .user_agent("amberite-core/0.1")
            .build()?;
        let broadcaster = EventBroadcaster::new();
        let jwks_cache = JwksCache::new(http.clone());

        let instance_store = Arc::new(InstanceRepo::new(pool.clone()));
        let modpack_store = Arc::new(ModpackRepo::new(pool.clone()));

        // Generate first-run pairing code if not yet paired.
        let is_paired = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM core_config")
            .fetch_one(&pool)
            .await
            .unwrap_or(0) > 0;

        let pairing_code = if is_paired {
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
            pairing_code: tokio::sync::Mutex::new(pairing_code),
            instance_store,
            modpack_store,
        }))
    }

    /// JWKS URL derived from the stored supabase_url.
    pub async fn jwks_url(&self) -> Option<String> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT supabase_url FROM core_config WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();
        row.map(|(url,)| format!("{url}/auth/v1/.well-known/jwks.json"))
    }
}

fn generate_pairing_code() -> String {
    use rand::Rng;
    let n: u32 = rand::thread_rng().gen_range(100_000..=999_999);
    n.to_string()
}
