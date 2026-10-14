//! Startup and shutdown of the Core process. `copal run` lands here.

pub mod config;
pub mod paths;

use std::{net::SocketAddr, sync::Arc};

use color_eyre::eyre::{Result, WrapErr};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{api, db, network::tls::Identity};
use config::Config;
use paths::Paths;

/// Run Core until it is asked to stop: open the database, then serve the API over HTTPS.
pub async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("copal=info")),
        )
        .init();

    let config = Config::load()?;
    let paths = Paths::new(&config.data_dir);
    // Step 2 creates every folder with its owner and permissions; until then only `db/` exists.
    std::fs::create_dir_all(paths.database().parent().expect("db/ folder"))
        .wrap_err_with(|| {
            format!(
                "could not create the data folder {}. Set CORE_DATA_DIR to use another one",
                paths.root().display()
            )
        })?;

    let db = db::open(&paths.database()).await?;
    let identity = Identity::load_or_create(&paths)?;
    let state = Arc::new(api::AppState { db });

    let address = SocketAddr::new(config.bind, config.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .wrap_err_with(|| format!("could not listen on {address}"))?;
    info!(
        %address,
        data = %paths.root().display(),
        fingerprint = %identity.fingerprint,
        "Copal {} is running",
        env!("CARGO_PKG_VERSION")
    );
    tokio::select! {
        () = crate::network::tls::serve(listener, api::router(state), &identity) => {}
        () = exit_requested() => info!("stopping"),
    }
    Ok(())
}

/// Resolves on Ctrl+C, or a service manager's stop on Unix.
async fn exit_requested() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
