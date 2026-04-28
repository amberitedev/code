use std::{net::SocketAddr, sync::Arc};

use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod application;
mod config;
mod domain;
mod infrastructure;
mod ports;
mod presentation;

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("amberite_core=info".parse()?))
        .init();

    let config = config::Config::from_env();

    // Ensure the data directory exists
    tokio::fs::create_dir_all(&config.data_dir).await?;

    let db_path = config.data_dir.join("data.db");
    let pool = infrastructure::db::connect(&db_path).await?;

    // Run all pending migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    let port = config.port;
    let state = application::state::AppState::new(config, pool).await?;

    // Restore running instances in background
    tokio::spawn(application::instance_service::restore_instances(Arc::clone(&state)));

    let router = presentation::router::create_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("Amberite Core listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
