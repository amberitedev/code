use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod application;
mod config;
mod domain;
mod infrastructure;
mod ports;
mod presentation;

/// Amberite Core — self-hosted Minecraft server manager.
#[derive(Parser)]
#[command(name = "amberite-core", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the HTTP API server (default when no subcommand is given).
    Run,
    /// Validate config and database connectivity without starting the server.
    Check,
    /// Apply all pending database migrations and exit.
    Migrate,
    /// Print the version string and exit.
    Version,
    /// Remove pairing data so Core can be re-paired.
    /// The next `run` will generate a fresh pairing code.
    ResetPairing,
}

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Run) {
        Command::Version => {
            println!("amberite-core {}", env!("CARGO_PKG_VERSION"));
        }
        Command::Migrate => {
            init_tracing();
            let config = config::Config::from_env();
            tokio::fs::create_dir_all(&config.data_dir).await?;
            let db_path = config.data_dir.join("data.db");
            let pool = infrastructure::db::connect(&db_path).await?;
            sqlx::migrate!("./migrations").run(&pool).await?;
            println!("Migrations applied successfully.");
        }
        Command::Check => {
            init_tracing();
            let config = config::Config::from_env();
            tokio::fs::create_dir_all(&config.data_dir).await?;
            let db_path = config.data_dir.join("data.db");
            let pool = infrastructure::db::connect(&db_path).await?;
            sqlx::migrate!("./migrations").run(&pool).await?;
            let paired: bool = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM core_config",
            )
            .fetch_one(&pool)
            .await
            .unwrap_or(0)
                > 0;
            println!("Config  : OK (data_dir = {})", config.data_dir.display());
            println!("Database: OK ({})", db_path.display());
            println!(
                "Paired  : {}",
                if paired { "yes" } else { "no — run to pair" }
            );
        }
        Command::ResetPairing => {
            init_tracing();
            let config = config::Config::from_env();
            let db_path = config.data_dir.join("data.db");
            let pool = infrastructure::db::connect(&db_path).await?;
            sqlx::query("DELETE FROM core_config WHERE id = 1")
                .execute(&pool)
                .await?;
            println!(
                "Pairing reset. Restart Core to generate a new pairing code."
            );
        }
        Command::Run => {
            init_tracing();
            run_server().await?;
        }
    }

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::from_default_env()
                .add_directive("amberite_core=info".parse().unwrap()),
        )
        .init();
}

async fn run_server() -> color_eyre::eyre::Result<()> {
    let config = config::Config::from_env();

    tokio::fs::create_dir_all(&config.data_dir).await?;

    let db_path = config.data_dir.join("data.db");
    let pool = infrastructure::db::connect(&db_path).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let port = config.port;
    let bind_host = config.bind_host.clone();
    let state = application::state::AppState::new(config, pool).await?;

    tokio::spawn(application::instance_service::restore_instances(
        Arc::clone(&state),
    ));
    tokio::spawn(gc_ws_tickets(Arc::clone(&state)));
    tokio::spawn(application::backup_scheduler::run_backup_scheduler(
        Arc::clone(&state),
    ));
    tokio::spawn(application::pairing_service::register_pairing_core(
        Arc::clone(&state),
    ));

    let router = presentation::router::create_router(state);
    let host: IpAddr = bind_host.parse()?;
    let addr = SocketAddr::new(host, port);

    info!("Amberite Core listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

/// SEC-02: Drain expired WebSocket tickets every 5 minutes to prevent unbounded growth.
async fn gc_ws_tickets(state: Arc<application::state::AppState>) {
    use std::time::Instant;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
        state
            .ws_tickets
            .retain(|_, t| t.expires_at > Instant::now());
    }
}
