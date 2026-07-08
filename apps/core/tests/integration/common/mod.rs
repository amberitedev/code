//! Shared test infrastructure for all integration tests.
//!
//! Each integration test file declares `mod common;` at the top to pull this in.
//! Unused-item warnings are suppressed — not every test file uses every helper.

#![allow(dead_code)]

use std::sync::Arc;

use copal::{
    application::{installation_service::installation_dir, state::AppState},
    config::Config,
    domain::instance::{InstanceId, InstanceInstallStatus},
    domain::server_installation::InstallationId,
    infrastructure::process::mock_spawner::MockSpawner,
    presentation::router::create_router,
};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use sqlx::sqlite::SqliteConnectOptions;

// ──────────────────────────────────────────────────────────────────────────────
// TestApp
// ──────────────────────────────────────────────────────────────────────────────

/// A running instance of Copal bound to an ephemeral port.
/// Dropped at test end: the TempDir is deleted and the server task is cancelled.
pub struct TestApp {
    pub base_url: String,
    pub client: reqwest::Client,
    pub state: Arc<AppState>,
    /// The first-run pairing code, if this app was spawned in prod (unpaired) mode.
    /// `None` for dev-mode spawns (pairing is skipped) and already-paired spawns.
    pub pairing_code: Option<String>,
    pub local_setup_secret: Option<String>,
    /// Keeps the temp directory alive until the TestApp is dropped.
    _data_dir: tempfile::TempDir,
}

impl TestApp {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Unpaired core, dev_mode = true (uses the explicit dev bearer token).
    pub async fn spawn() -> Self {
        let (pool, data_dir) = Self::db_and_dir().await;
        let config = Self::dev_config(data_dir.path().to_path_buf());
        let state = AppState::new(config, pool).await.unwrap();
        Self::start(state, data_dir, None, None).await
    }

    /// Unpaired core, dev_mode = true, with MockSpawner injected.
    /// Use this for instance lifecycle tests (start/stop/kill/command).
    pub async fn spawn_with_mock() -> Self {
        let (pool, data_dir) = Self::db_and_dir().await;
        let config = Self::dev_config(data_dir.path().to_path_buf());
        let state =
            AppState::new_with_spawner(config, pool, Arc::new(MockSpawner))
                .await
                .unwrap();
        Self::start(state, data_dir, None, None).await
    }

    /// Unpaired core, dev_mode = false (real JWT enforcement).
    /// No `core_config` row → any request with no Bearer token → 401.
    /// The generated pairing code is available as `app.pairing_code`.
    pub async fn spawn_prod_unpaired() -> Self {
        let (pool, data_dir) = Self::db_and_dir().await;
        let config = Config {
            dev_mode: false,
            ..Self::dev_config(data_dir.path().to_path_buf())
        };
        let state = AppState::new(config, pool).await.unwrap();
        let pairing_code = state.pairing_code.lock().await.clone();
        let local_setup_secret = state.local_setup_secret.lock().await.clone();
        Self::start(state, data_dir, pairing_code, local_setup_secret).await
    }

    /// Already-paired core, dev_mode = true.
    /// Pre-inserts `core_config` to suppress the pairing code banner.
    pub async fn spawn_paired() -> Self {
        let (pool, data_dir) = Self::db_and_dir().await;
        sqlx::query(
			"INSERT INTO core_config (id, supabase_url, convex_url, auth_jwks_url, owner_user_id, paired_at) \
			 VALUES (1, ?, ?, ?, ?, ?)",
		)
		.bind("https://test.convex.cloud")
		.bind("https://test.convex.cloud")
		.bind("https://auth.test/.well-known/jwks.json")
        .bind("test-owner")
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

        let config = Config {
            convex_url: "https://test.convex.cloud".to_string(),
            ..Self::dev_config(data_dir.path().to_path_buf())
        };
        let state = AppState::new(config, pool).await.unwrap();
        Self::start(state, data_dir, None, None).await
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Format a path relative to this app's base URL.
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    // ── Private setup ─────────────────────────────────────────────────────────

    /// Create a fresh temp dir + migrated SQLite pool.
    async fn db_and_dir() -> (sqlx::SqlitePool, tempfile::TempDir) {
        let data_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = data_dir.path().join("test.db");
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(options.foreign_keys(true))
            .await
            .expect("failed to open test SQLite");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migration failed");
        (pool, data_dir)
    }

    fn dev_config(data_dir: std::path::PathBuf) -> Config {
        Config {
            data_dir,
            convex_url: "https://test.convex.cloud".to_string(),
            public_url: "http://127.0.0.1".to_string(),
            port: 0,
            bind_host: "127.0.0.1".to_string(),
            allowed_origin: "*".to_string(),
            dev_mode: true,
            no_auth: false,
            sync_retain_count: 10,
        }
    }

    async fn start(
        state: Arc<AppState>,
        data_dir: tempfile::TempDir,
        pairing_code: Option<String>,
        local_setup_secret: Option<String>,
    ) -> Self {
        let router = create_router(Arc::clone(&state));
        let client = if state.config.dev_mode {
            let mut headers = HeaderMap::new();
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_static("Bearer dev:dev-owner"),
            );
            reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .expect("failed to build authenticated test client")
        } else {
            reqwest::Client::new()
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind test listener");
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            client,
            state,
            pairing_code,
            local_setup_secret,
            _data_dir: data_dir,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Fixtures
// ──────────────────────────────────────────────────────────────────────────────

/// Default JSON body for POST /instances.
pub fn default_create_body() -> serde_json::Value {
    serde_json::json!({
        "name": "test-server",
        "game_version": "1.21.1",
        "loader": "vanilla",
        "port": 25565,
        "memory": { "min_mb": 512, "max_mb": 1024 }
    })
}

/// Create one instance and return its ID string.
pub async fn create_test_instance(app: &TestApp) -> String {
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instances")
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    let mut body = default_create_body();
    body["port"] = serde_json::json!(25565 + existing);
    let res = app
        .client
        .post(app.url("/instances"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "create_test_instance failed: {}",
        res.status()
    );
    let id = res.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let parsed = id.parse::<InstanceId>().unwrap();
    app.state
        .instance_store
        .update_install_status(&parsed, InstanceInstallStatus::Ready)
        .await
        .unwrap();
    let record = app.state.instance_store.get(&parsed).await.unwrap();
    let launch_dir = record
        .installation_id
        .as_ref()
        .map(|id| installation_dir(&app.state, &InstallationId(id.clone())))
        .unwrap_or_else(|| std::path::PathBuf::from(&record.data_dir));
    tokio::fs::create_dir_all(&launch_dir).await.unwrap();
    tokio::fs::write(launch_dir.join("server.jar"), b"dummy")
        .await
        .unwrap();
    id
}
