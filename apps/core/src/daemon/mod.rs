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

use std::{
    fs::File,
    future::Future,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

use color_eyre::eyre::{Result, WrapErr, bail, eyre};
use tracing::{info, warn};
use tracing_subscriber::{
    EnvFilter, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::{
    access, api, db,
    network::{forward::Network, tls::Identity},
    servers::Servers,
    update::Updater,
};
use accounts::Accounts;
use config::Config;
use install::InstallRecord;
use paths::Paths;

/// `copal run`: the launcher (see `update::launcher`), which runs Core as its child. `service`
/// is set by the Windows service manager, which starts Copal that way.
pub fn run(service: bool) -> Result<()> {
    if service {
        #[cfg(windows)]
        return service::dispatch();
        #[cfg(not(windows))]
        bail!(
            "--service is for the Windows service manager; systemd and launchd use copal run"
        );
    }
    let paths = Paths::new(config::data_dir());
    #[cfg(unix)]
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o027));
    logging(&paths, "launcher");
    let code = tokio::runtime::Runtime::new()?.block_on(
        crate::update::launcher::run(&paths, false, exit_requested()),
    )?;
    std::process::exit(code)
}

/// How Core ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Stopped,
    /// An update or rollback: the launcher starts the active version.
    Restart,
}

/// `copal core`: Core itself, as the launcher runs it. It stops when its stdin closes, which is
/// how the launcher stops it, or on Ctrl+C.
pub fn core() -> Result<()> {
    // A thread of its own: a blocking read must not hold the runtime open at exit.
    let (closed, stdin_closed) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
        let _ = closed.send(());
    });
    let stop = async {
        tokio::select! {
            _ = stdin_closed => {}
            () = exit_requested() => {}
        }
    };
    match tokio::runtime::Runtime::new()?.block_on(serve(stop))? {
        Exit::Stopped => Ok(()),
        Exit::Restart => std::process::exit(crate::update::launcher::RESTART),
    }
}

/// Run Core until `stop` resolves or an update asks for a restart: adopt running servers, then
/// serve the API over HTTPS and the local socket. Servers keep running after Core stops.
pub async fn serve(stop: impl Future<Output = ()>) -> Result<Exit> {
    let config = Config::load()?;
    let paths = Paths::new(&config.data_dir);
    #[cfg(unix)]
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o027));
    let record = InstallRecord::load(&paths)?;
    let installed = record.is_some();
    let owner = match record {
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
    logging(&paths, "copal");
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
    let storage = Arc::new(crate::store::Storage::new(
        db.clone(),
        paths.clone(),
        core_id.clone(),
        config.backend_url.clone(),
        installed,
        Arc::clone(&access.owed),
    ));
    storage.start();
    let servers = Servers::open(paths.clone(), db.clone()).await?;
    let backups = crate::backups::Backups::open(
        paths.clone(),
        db.clone(),
        Arc::clone(&servers),
    )
    .await?;
    let content = crate::content::Content::open(
        paths.clone(),
        db.clone(),
        Arc::clone(&servers),
        Arc::clone(&backups),
    )?;
    let update = Arc::new(Updater::new(
        paths.clone(),
        db.clone(),
        http.clone(),
        config.update_url.clone(),
    ));
    tokio::spawn(Arc::clone(&update).auto(Arc::clone(&servers)));
    tokio::spawn(crate::servers::restarts::run(Arc::clone(&servers)));
    let network = Arc::new(Network::new(
        db.clone(),
        config.port,
        installed,
        config.forward_ports,
    ));
    tokio::spawn(Arc::clone(&network).run(Arc::clone(&servers)));
    let state = Arc::new(api::AppState {
        db: db.clone(),
        servers,
        access: Arc::clone(&access),
        content,
        backups,
        uploads: api::files::Uploads::default(),
        update: Arc::clone(&update),
        network,
        storage: Arc::clone(&storage),
    });
    let router = api::router(Arc::clone(&state));
    tokio::spawn(api::content::apply_pending_on_stop(Arc::clone(&state)));
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
                storage,
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
    let listener = listen(address)
        .wrap_err_with(|| format!("could not listen on {address}"))?;
    // On every IPv4 address by default, so on every IPv6 one too. A machine without IPv6 skips it.
    let v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), config.port);
    let v6 = (config.bind == IpAddr::V4(Ipv4Addr::UNSPECIFIED))
        .then(|| listen(v6))
        .and_then(|result| {
            result
                .inspect_err(|error| warn!(%error, "not listening on IPv6"))
                .ok()
        });
    let v6_router = router.clone();
    let serve_v6 = async {
        match v6 {
            Some(v6) => {
                crate::network::tls::serve(v6, v6_router, &identity).await;
            }
            None => std::future::pending().await,
        }
    };
    info!(
        %address,
        data = %paths.root().display(),
        fingerprint = %identity.fingerprint,
        "Copal {} is running",
        env!("CARGO_PKG_VERSION")
    );
    tokio::spawn(answer_once(paths.clone()));
    tokio::select! {
        () = crate::network::tls::serve(listener, router.clone(), &identity) => {}
        () = serve_v6 => {}
        result = access::local::serve(&paths, &owner, router) => result?,
        () = update.restart.notified() => {
            info!("restarting into another version; servers keep running");
            return Ok(Exit::Restart);
        }
        () = stop => info!("stopping; servers keep running"),
    }
    Ok(Exit::Stopped)
}

/// A TCP listener on `address`. An IPv6 one takes IPv6 only, so it can sit next to the IPv4
/// one on every OS.
fn listen(address: SocketAddr) -> std::io::Result<tokio::net::TcpListener> {
    let socket = socket2::Socket::new(
        socket2::Domain::for_address(address),
        socket2::Type::STREAM,
        None,
    )?;
    if address.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    // Lets a restarted Core take its port back at once. Windows lets another process share
    // the port with it, so there it stays off.
    #[cfg(unix)]
    socket.set_reuse_address(true)?;
    socket.bind(&address.into())?;
    socket.listen(1024)?;
    socket.set_nonblocking(true)?;
    tokio::net::TcpListener::from_std(socket.into())
}

/// Ask this Core for `/health` over the local socket, so the first answer, which writes the
/// health marker, comes even when nobody else asks.
async fn answer_once(paths: Paths) {
    let client = crate::cli::Client::at(paths);
    for _ in 0..20 {
        if client.get::<serde_json::Value>("/health").await.is_ok() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    warn!("Core could not reach its own local socket");
}

/// Log to the console and to a file in `logs/` named `prefix` that starts over daily, keeping a
/// week.
fn logging(paths: &Paths, prefix: &str) {
    let file = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(prefix)
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
