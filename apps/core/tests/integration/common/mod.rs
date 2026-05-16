//! Shared test infrastructure for all integration tests.
//!
//! Each integration test file declares `mod common;` at the top to pull this in.
//! Unused-item warnings are suppressed — not every test file uses every helper.

#![allow(dead_code)]

use std::sync::Arc;

use amberite_core::{
    application::state::AppState, config::Config,
    infrastructure::process::mock_spawner::MockSpawner,
    presentation::router::create_router,
};
use sqlx::sqlite::SqliteConnectOptions;

// ──────────────────────────────────────────────────────────────────────────────
// TestApp
// ──────────────────────────────────────────────────────────────────────────────

/// A running instance of Amberite Core bound to an ephemeral port.
/// Dropped at test end: the TempDir is deleted and the server task is cancelled.
pub struct TestApp {
    pub base_url: String,
    pub client: reqwest::Client,
    /// The first-run pairing code, if this app was spawned in prod (unpaired) mode.
    /// `None` for dev-mode spawns (pairing is skipped) and already-paired spawns.
    pub pairing_code: Option<String>,
    /// Keeps the temp directory alive until the TestApp is dropped.
    _data_dir: tempfile::TempDir,
}

impl TestApp {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Unpaired core, dev_mode = true (no JWT required).
    pub async fn spawn() -> Self {
        let (pool, data_dir) = Self::db_and_dir().await;
        let config = Self::dev_config(data_dir.path().to_path_buf());
        let state = AppState::new(config, pool).await.unwrap();
        Self::start(state, data_dir, None).await
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
        Self::start(state, data_dir, None).await
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
        Self::start(state, data_dir, pairing_code).await
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
            convex_url: Some("https://test.convex.cloud".to_string()),
            ..Self::dev_config(data_dir.path().to_path_buf())
        };
        let state = AppState::new(config, pool).await.unwrap();
        Self::start(state, data_dir, None).await
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
        let pool = sqlx::SqlitePool::connect_with(options)
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
            convex_url: None,
            public_url: None,
            port: 0,
            bind_host: "127.0.0.1".to_string(),
            allowed_origin: "*".to_string(),
            dev_mode: true,
        }
    }

    async fn start(
        state: Arc<AppState>,
        data_dir: tempfile::TempDir,
        pairing_code: Option<String>,
    ) -> Self {
        let router = create_router(Arc::clone(&state));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind test listener");
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            client: reqwest::Client::new(),
            pairing_code,
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
    let res = app
        .client
        .post(app.url("/instances"))
        .json(&default_create_body())
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "create_test_instance failed: {}",
        res.status()
    );
    res.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}
