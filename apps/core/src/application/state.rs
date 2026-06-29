use std::{
    path::PathBuf,
    sync::{atomic::AtomicU32, Arc},
    time::{Duration, Instant},
};

use dashmap::DashMap;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    config::Config,
    domain::instance::InstanceId,
    infrastructure::{
        auth::jwks::JwksCache,
        db::{
            installation_repo::InstallationRepo, instance_repo::InstanceRepo,
            java_repo::JavaRepo, modpack_repo::ModpackRepo,
        },
        events::EventBroadcaster,
        process::{instance_actor::InstanceHandle, std_spawner::StdSpawner},
    },
    ports::{
        installation_store::InstallationStore, instance_store::InstanceStore,
        java_store::JavaStore, modpack_store::ModpackStore,
        process_spawner::AnySpawner,
    },
};

/// Duration that a terminal pairing code remains valid.
pub const PAIRING_WINDOW: Duration = Duration::from_secs(15 * 60);

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
    /// JWKS cache for auth JWT validation.
    pub jwks_cache: JwksCache,
    /// In-memory short-lived WebSocket tickets.
    pub ws_tickets: DashMap<String, WsTicket>,
    /// In-memory short-lived file download tokens (issued by GET /instances/:id/fs/url).
    pub fs_download_tokens: DashMap<String, FsDownloadToken>,
    /// In-memory resumable upload sessions.
    pub fs_upload_sessions: DashMap<String, FsUploadSession>,
    /// First-run pairing code (cleared after pairing).
    pub pairing_code: tokio::sync::Mutex<Option<String>>,
    /// Expiration instant for the first-run pairing code.
    pub pairing_code_expires_at: tokio::sync::Mutex<Option<Instant>>,
    /// Local one-time setup secret for app-launched Cores.
    pub local_setup_secret: tokio::sync::Mutex<Option<String>>,
    /// SEC-01: counts wrong pairing-code attempts; locked out after MAX_PAIRING_ATTEMPTS.
    pub wrong_pairing_attempts: AtomicU32,
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
        let jwks_cache = JwksCache::new(http.clone());

        let instance_store = Arc::new(InstanceRepo::new(pool.clone()));
        let installation_store = Arc::new(InstallationRepo::new(pool.clone()));
        let java_store = Arc::new(JavaRepo::new(pool.clone()));
        let modpack_store = Arc::new(ModpackRepo::new(pool.clone()));
        let core_id = load_or_create_core_id(&pool).await?;

        // Generate first-run pairing code if not yet paired.
        let is_paired =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM core_config")
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
                > 0;

        if is_paired {
            sqlx::query(
                "UPDATE core_config SET core_id = ? WHERE id = 1 AND core_id IS NULL",
            )
            .bind(&core_id)
            .execute(&pool)
            .await
            .ok();
        }

        let (pairing_code, pairing_code_expires_at, local_setup_secret) =
            if is_paired {
                (None, None, None)
            } else {
                let code = generate_pairing_code();
                let secret = generate_setup_secret();
                write_local_setup_secret(&config.data_dir, &secret).await?;
                (
                    Some(code),
                    Some(Instant::now() + PAIRING_WINDOW),
                    Some(secret),
                )
            };

        Ok(Arc::new(Self {
            pool,
            http,
            config,
            core_id,
            instances: DashMap::new(),
            instance_operation_locks: DashMap::new(),
            broadcaster,
            jwks_cache,
            ws_tickets: DashMap::new(),
            fs_download_tokens: DashMap::new(),
            fs_upload_sessions: DashMap::new(),
            pairing_code: tokio::sync::Mutex::new(pairing_code),
            pairing_code_expires_at: tokio::sync::Mutex::new(
                pairing_code_expires_at,
            ),
            local_setup_secret: tokio::sync::Mutex::new(local_setup_secret),
            wrong_pairing_attempts: AtomicU32::new(0),
            instance_store,
            installation_store,
            java_store,
            modpack_store,
            spawner,
        }))
    }

    /// JWKS URL written during setup for the active auth provider.
    pub async fn jwks_url(&self) -> Option<String> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT auth_jwks_url FROM core_config WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();
        row.map(|(url,)| url)
    }

    /// Expected JWT audience for the active auth provider.
    pub async fn auth_audience(&self) -> Option<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT auth_audience FROM core_config WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
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

pub async fn write_local_setup_secret(
    data_dir: &std::path::Path,
    secret: &str,
) -> color_eyre::eyre::Result<()> {
    tokio::fs::create_dir_all(data_dir).await?;
    let path = data_dir.join(".setup_secret");
    tokio::fs::write(&path, secret).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = tokio::fs::metadata(&path).await?.permissions();
        permissions.set_mode(0o600);
        tokio::fs::set_permissions(&path, permissions).await?;
    }

    Ok(())
}

pub fn generate_pairing_code() -> String {
    use rand::Rng;

    const PAIRING_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    let code: String = (0..8)
        .map(|_| {
            let index = rng.gen_range(0..PAIRING_ALPHABET.len());
            (PAIRING_ALPHABET[index] as char).to_ascii_lowercase()
        })
        .collect();
    code
}

pub fn format_pairing_code(code: &str) -> String {
    format!(
        "{}-{}",
        code[..4].to_ascii_uppercase(),
        code[4..].to_ascii_uppercase()
    )
}

pub fn generate_setup_secret() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::{format_pairing_code, generate_pairing_code};

    #[test]
    fn pairing_codes_are_stored_canonically_and_formatted_for_display() {
        let code = generate_pairing_code();
        let display = format_pairing_code(&code);
        let (first, second) = display
            .split_once('-')
            .expect("display code should include a dash");

        assert_eq!(code.len(), 8);
        assert!(code.chars().all(|character| character.is_ascii_lowercase()
            || character.is_ascii_digit()));
        assert_eq!(first.len(), 4);
        assert_eq!(second.len(), 4);
        assert!(display.chars().all(|character| character == '-'
            || character.is_ascii_uppercase()
            || character.is_ascii_digit()));
    }
}
