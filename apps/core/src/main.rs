//! Lodestone Core V2 - Main entry point.
//! Wires Domain, Application, Infrastructure, Presentation layers.

mod domain;
mod application;
mod infrastructure;
mod presentation;
mod theseus;

use std::path::PathBuf;
use std::sync::Arc;
use clap::Parser;
use sqlx::{SqlitePool, Row};
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use tracing::info;
use rand::RngCore;
use thiserror::Error;

use presentation::router::create_router;
use application::registry::ServiceRegistry;
use application::auth_service::AuthService;
use application::instance_service::InstanceService;
use application::macro_engine::MacroEngine;
use infrastructure::sqlite_repo::SqliteRepo;
use infrastructure::pty_spawner::PtySpawner;
use infrastructure::deno_runtime::DenoRuntime;

#[derive(Error, Debug)]
enum AppError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("IO error: {0}")]
    Io(String),
}

type Result<T> = std::result::Result<T, AppError>;

#[derive(Parser, Debug)]
#[command(name = "lodestone-core")]
#[command(about = "Lodestone Core V2")]
struct Args {
    #[arg(long)]
    lodestone_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = color_eyre::install();

    let args = Args::parse();
    let lodestone_path = args.lodestone_path
        .or_else(|| std::env::var("LODESTONE_PATH").ok().map(PathBuf::from))
        .unwrap_or_else(|| {
            std::env::current_dir().expect("Failed to get current dir")
        });

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from("lodestone_core=debug"))
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set subscriber");

    info!("Lodestone Core V2 starting...");
    info!("Data directory: {}", lodestone_path.display());

    dotenvy::from_path(lodestone_path.join(".env")).ok();

    let db_path = lodestone_path.join("data.db");
    let db_url = format!("sqlite://{}", db_path.display());

    let pool = SqlitePool::connect(&db_url).await.map_err(|e| {
        AppError::Database(e.to_string())
    })?;
    info!("Connected to database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await.map_err(|e| {
            AppError::Database(e.to_string())
        })?;
    info!("Migrations applied");

    // Initialize Theseus state for modpack management
    let theseus_base_dir = lodestone_path.join(".lodestone");
    theseus::state::State::init(theseus_base_dir, pool.clone()).await
        .map_err(|e| AppError::Io(e.to_string()))?;
    info!("Theseus state initialized");

    let paseto_key = load_paseto_key(&pool).await?;

    let sqlite_repo = Arc::new(SqliteRepo::new(pool).await);
    let os_manager = Arc::new(PtySpawner::new());
    let deno_runtime = Arc::new(DenoRuntime::new().unwrap());

    let auth_service = Arc::new(AuthService::new(sqlite_repo.clone(), paseto_key));
    let instance_service = Arc::new(InstanceService::new(sqlite_repo.clone(), os_manager));
    let macro_engine = Arc::new(MacroEngine::new(deno_runtime));

    let registry = ServiceRegistry::new(auth_service, instance_service, macro_engine);
    let router = create_router(registry);

    let addr: std::net::SocketAddr = ([0, 0, 0, 0], 16662).into();
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        AppError::Io(e.to_string())
    })?;
    axum::serve(listener, router).await.map_err(|e| {
        AppError::Io(e.to_string())
    })?;

    Ok(())
}

async fn load_paseto_key(pool: &SqlitePool) -> Result<[u8; 32]> {
    let row = sqlx::query("SELECT key FROM paseto_key WHERE id = 1")
        .fetch_optional(pool)
        .await.map_err(|e| {
            AppError::Database(e.to_string())
        })?;

    if let Some(row) = row {
        let key_bytes: Vec<u8> = row.get("key");
        if key_bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&key_bytes);
            return Ok(key);
        }
    }

    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);

    sqlx::query("INSERT INTO paseto_key (id, key) VALUES (1, ?)")
        .bind(&key as &[u8])
        .execute(pool)
        .await.map_err(|e| {
            AppError::Database(e.to_string())
        })?;

    Ok(key)
}
