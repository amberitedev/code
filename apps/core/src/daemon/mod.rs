//! Startup and shutdown of the Core process. `copal run` lands here, in the foreground or under
//! the service manager.

pub mod accounts;
pub mod config;
pub mod folders;
pub mod install;
pub mod paths;
pub mod service;
#[cfg(windows)]
pub mod win;

use std::{fs::File, future::Future, net::SocketAddr, sync::Arc};

use color_eyre::eyre::{Result, WrapErr, bail, eyre};
use tracing::{info, warn};
use tracing_subscriber::{
    EnvFilter, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::{access, api, db, network::tls::Identity, servers::Servers};
use accounts::Accounts;
use config::Config;
use install::InstallRecord;
use paths::Paths;

/// `copal run`. `service` is set by the Windows service manager, which starts Core that way.
pub fn run(service: bool) -> Result<()> {
    if service {
        #[cfg(windows)]
        return service::dispatch();
        #[cfg(not(windows))]
        bail!(
            "--service is for the Windows service manager; systemd and launchd use copal run"
        );
    }
    tokio::runtime::Runtime::new()?.block_on(serve(exit_requested()))
}

/// Run Core until `stop` resolves: adopt running servers, then serve the API over HTTPS and the
/// local socket. Servers keep running after Core stops.
pub async fn serve(stop: impl Future<Output = ()>) -> Result<()> {
    let config = Config::load()?;
    let paths = Paths::new(&config.data_dir);
    #[cfg(unix)]
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o027));
    let owner = match InstallRecord::load(&paths)? {
        Some(record) => {
            let accounts = Accounts::find()?.ok_or_else(|| {
                eyre!("the copal accounts are missing. Run copal install again")
            })?;
            if !accounts.is_current()? {
                bail!(
                    "Copal is installed here, so Core runs as the {} account. {}",
                    accounts::CORE,
                    if cfg!(windows) {
                        "Start the copal service instead"
                    } else {
                        "Start the copal service, or run: sudo -u copal copal run"
                    }
                );
            }
            record.owner
        }
        // Development: the account running Core is the owner, and servers run as it too.
        None => accounts::current_id()?,
    };
    folders::create(&paths, None)?;
    logging(&paths);
    let _lock = lock(&paths)?;

    let db = db::open(&paths.database()).await?;
    let core_id = db::core_id(&db).await?;
    let identity = Identity::load_or_create(&paths)?;
    let http = reqwest::Client::builder()
        .user_agent(concat!("copal/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()?;
    let keys = access::keys::Keys::load(
        db.clone(),
        http.clone(),
        config.backend_url.as_deref(),
    )
    .await?;
    let access = Arc::new(access::Access::new(keys));
    let servers = Servers::open(paths.clone(), db.clone()).await?;
    let state = Arc::new(api::AppState {
        db: db.clone(),
        servers,
        access: Arc::clone(&access),
    });
    let router = api::router(Arc::clone(&state));
    match config.backend_url.clone() {
        Some(backend_url) => {
            tokio::spawn(access::sync::run(Arc::new(access::sync::Sync {
                db,
                http,
                backend_url,
                core_id,
                cert_fingerprint: identity.fingerprint.clone(),
                port: config.port,
                bind: config.bind,
                access: Arc::clone(&access),
            })));
            tokio::spawn(
                async move { state.access.keys.refresh_daily().await },
            );
        }
        None => warn!(
            "no backend is set (ACCOUNT_API_URL), so this Core cannot pair and only the copal CLI can use it"
        ),
    }

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
        () = crate::network::tls::serve(listener, router.clone(), &identity) => {}
        result = access::local::serve(&paths, &owner, router) => result?,
        () = stop => info!("stopping; servers keep running"),
    }
    Ok(())
}

/// Log to the console and to a file in `logs/` that starts over daily, keeping a week.
fn logging(paths: &Paths) {
    let file = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("copal")
        .filename_suffix("log")
        .max_log_files(7)
        .build(paths.logs())
        .ok();
    let _ = tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("copal=info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(file.map(|file| {
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file)
        }))
        .try_init();
}

/// Hold the lock that keeps a second Core off this data folder, until the process exits.
fn lock(paths: &Paths) -> Result<File> {
    let path = paths.core_lock();
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .wrap_err_with(|| format!("could not open {}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => {
            bail!("another Core already runs on {}", paths.root().display())
        }
        Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_core_cannot_take_the_data_folder() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        folders::create(&paths, None).unwrap();
        let first = lock(&paths).unwrap();
        assert!(lock(&paths).is_err());
        drop(first);
        assert!(lock(&paths).is_ok());
    }
}
