//! The shared file pool's store: encrypted objects named by hash, kept
//! for versions a backend-signed ticket names, served to whoever holds a fetch ticket.
//!
//! It runs as `copal store`, a child process Copal starts and supervises, and is reached only
//! through Copal: Copal forwards `/store/...` from its own address and port to the store's HTTP
//! listener on `127.0.0.1`, unread. The store has no identity of its own: no backend record, key,
//! or sign-in, no database, and neither Copal's backend key nor its TLS key, so a compromise of
//! it yields none. What it holds reaches the backend in Copal's reports.
//!
//! Copal manages it over its stdin and stdout ([`local`]): the store says where it listens, then
//! answers status, drop, and config calls. Only Copal holds the pipes, so the calls need no
//! token. Closing stdin stops the store. stderr is its log, which Copal copies into its own.

pub mod keys;
mod limits;
pub mod local;
mod objects;
pub mod pull;
mod routes;
mod tar;
mod tickets;

/// Where every reqwest client in the store starts. reqwest is built without a TLS provider so
/// the store stays on ring, and it panics without one installed; installing it here covers
/// tests too.
pub(crate) fn http() -> reqwest::ClientBuilder {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
}

use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use color_eyre::eyre::{Result, WrapErr};
use contracts::{ApiError, ErrorCode, backend::cores::HeldVersion};
use tracing::{info, warn};

use keys::{JsonFile, Keys};
use limits::Limits;
use local::{Caps, Status};
pub use objects::Hash;
use objects::{Index, Layout, ListState, VersionList};

const GC_EVERY: Duration = Duration::from_secs(10 * 60);

/// `copal store`, as Copal starts it.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The store's own folder.
    #[arg(long)]
    pub data_dir: PathBuf,
    /// The audience pool tickets must name.
    #[arg(long)]
    pub core_id: String,
    /// The backend whose keys sign tickets, or `none`.
    #[arg(long)]
    pub backend_url: String,
    /// Bytes the store may hold.
    #[arg(long)]
    pub cap: u64,
    /// Bytes it may serve a day.
    #[arg(long)]
    pub egress_cap: u64,
    /// Let a pull copy from a private address, as Cores do in development.
    #[arg(long)]
    pub allow_private_sources: bool,
}

/// An I/O failure as the answer a caller sees; the cause goes to the log.
pub(crate) fn io_error(error: std::io::Error) -> ApiError {
    warn!(%error, "the store failed");
    ApiError::new(
        ErrorCode::InternalError,
        "The store failed. Its log says why.",
    )
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Everything the store's routes reach.
pub struct Store {
    layout: Layout,
    core_id: String,
    keys: Keys<JsonFile>,
    allow_private_sources: bool,
    /// Pulls from other Cores. One for the life of the store.
    client: sharing_client::pool::StoreClient,
    caps: Mutex<Caps>,
    index: Mutex<Index>,
    limits: Limits,
}

impl Store {
    /// The store in `layout`'s folder, creating it when missing.
    fn open(
        layout: Layout,
        core_id: String,
        keys: Keys<JsonFile>,
        allow_private_sources: bool,
        caps: Caps,
    ) -> std::io::Result<Self> {
        layout.create()?;
        let index = Index::load(&layout)?;
        Ok(Self {
            limits: Limits::load(layout.egress()),
            layout,
            core_id,
            keys,
            allow_private_sources,
            client: sharing_client::pool::StoreClient::direct(concat!(
                "copal-store/",
                env!("CARGO_PKG_VERSION")
            )),
            caps: Mutex::new(caps),
            index: Mutex::new(index),
        })
    }

    fn caps(&self) -> Caps {
        *self.caps.lock().expect("not poisoned")
    }

    fn set_caps(&self, caps: Caps) {
        *self.caps.lock().expect("not poisoned") = caps;
    }

    fn index(&self) -> std::sync::MutexGuard<'_, Index> {
        self.index.lock().expect("not poisoned")
    }

    fn is_stored(&self, root: &Hash) -> bool {
        self.index().stored.contains_key(root)
    }

    /// Take `list` as `root`'s, checked against `root`, `max_bytes` (a store ticket's), and the
    /// caps, and keep it pending until every object is present. Returns the objects missing.
    async fn begin(
        &self,
        root: &Hash,
        list: VersionList,
        max_bytes: Option<u64>,
    ) -> Result<Vec<Hash>, ApiError> {
        let invalid =
            |description| ApiError::new(ErrorCode::InvalidInput, description);
        list.check(root).map_err(invalid)?;
        if max_bytes.is_some_and(|max| list.bytes() > max) {
            return Err(invalid(
                "The version is larger than its ticket allows.",
            ));
        }
        let missing = objects::missing(&self.layout, &list)
            .await
            .map_err(io_error)?;
        let needed: u64 = list
            .objects
            .iter()
            .filter(|entry| missing.contains(&entry.hash))
            .map(|entry| entry.size)
            .sum();
        // The cap check and the reservation are one step under the index lock, so concurrent
        // uploads count each other. A later failure takes the reservation back.
        let cap = self.caps().cap;
        let list = Arc::new(list);
        let reserved = {
            let mut index = self.index();
            if index.used_with(Some(&list)) > cap {
                return Err(ApiError::new(
                    ErrorCode::DiskFull,
                    "This Core's storage for shared files is full.",
                ));
            }
            (!index.pending.contains_key(root)).then(|| {
                let at = now();
                index.pending.insert(root.clone(), (at, Arc::clone(&list)));
                at
            })
        };
        let unreserve = || {
            if reserved.is_some() {
                self.index().pending.remove(root);
            }
        };
        let free = sharing_client::disk::free(self.layout.root())
            .inspect_err(|_| unreserve())
            .map_err(io_error)?;
        if free < needed + contracts::limits::DISK_MARGIN_BYTES {
            unreserve();
            return Err(ApiError::new(
                ErrorCode::DiskFull,
                "This Core's disk is full.",
            ));
        }
        if let Some(at) = reserved {
            objects::write_atomic(
                &self.layout.list(root, ListState::Pending(at)),
                &serde_json::to_vec(list.as_ref()).expect("lists serialize"),
            )
            .await
            .inspect_err(|_| unreserve())
            .map_err(io_error)?;
        }
        Ok(missing)
    }

    /// Store `root`'s pending list once every object is present. Returns those still missing.
    async fn finish(&self, root: &Hash) -> Result<Vec<Hash>, ApiError> {
        let pending = self.index().pending.get(root).cloned();
        let Some((at, list)) = pending else {
            if self.is_stored(root) {
                return Ok(Vec::new());
            }
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "This upload was abandoned. Start it again.",
            ));
        };
        let missing = objects::missing(&self.layout, &list)
            .await
            .map_err(io_error)?;
        if !missing.is_empty() {
            return Ok(missing);
        }
        objects::write_atomic(
            &self.layout.list(root, ListState::Stored),
            &serde_json::to_vec(list.as_ref()).expect("lists serialize"),
        )
        .await
        .map_err(io_error)?;
        {
            let mut index = self.index();
            index.pending.remove(root);
            index.stored.insert(root.clone(), list);
        }
        let _ = tokio::fs::remove_file(
            self.layout.list(root, ListState::Pending(at)),
        )
        .await;
        info!(%root, "stored a version");
        Ok(Vec::new())
    }

    /// Stop holding `roots`. Their lists are kept as dropped for the grace period, so fetch
    /// tickets already out keep working; cleanup deletes the objects after.
    async fn drop_versions(&self, roots: &[Hash]) -> std::io::Result<()> {
        let at = now();
        for root in roots {
            let Some(list) = self.index().stored.get(root).cloned() else {
                continue;
            };
            tokio::fs::rename(
                self.layout.list(root, ListState::Stored),
                self.layout.list(root, ListState::Dropped(at)),
            )
            .await?;
            let mut index = self.index();
            index.stored.remove(root);
            index.dropped.push((root.clone(), at, list));
            info!(%root, "dropped a version");
        }
        Ok(())
    }

    fn status(&self) -> std::io::Result<Status> {
        let caps = self.caps();
        let (used, versions) = {
            let index = self.index();
            let versions = index
                .stored
                .iter()
                .map(|(root, list)| HeldVersion {
                    root: root.to_string(),
                    bytes: list.bytes(),
                })
                .collect();
            (index.used(), versions)
        };
        Ok(Status {
            cap: caps.cap,
            used,
            free: sharing_client::disk::free(self.layout.root())?,
            egress_cap: caps.egress_cap,
            egress_today: self.limits.egress_today(),
            versions,
        })
    }

    /// Delete what no list needs any more.
    async fn collect_garbage(self: &Arc<Self>) {
        let store = Arc::clone(self);
        let done = tokio::task::spawn_blocking(move || {
            objects::collect_garbage(&store.layout, &store.index, now())
        })
        .await;
        match done {
            Ok(Ok(())) => {}
            Ok(Err(error)) => warn!(%error, "could not clean up the store"),
            Err(error) => warn!(%error, "the store's cleanup stopped"),
        }
    }
}

/// `copal store`: serve until stdin closes, which is how Copal stops it.
pub async fn main(args: Args) -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sharing_storage=info".into()),
        )
        // Copal writes each line into its own log, which adds the time.
        .without_time()
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .try_init();
    let backend_url =
        Some(args.backend_url).filter(|url| !url.is_empty() && url != "none");
    let http = http()
        .user_agent(concat!("copal-store/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .build()?;
    let layout = Layout::new(&args.data_dir);
    let keys = Keys::load_file(layout.keys(), http, backend_url.as_deref());
    let store = Arc::new(
        Store::open(
            layout,
            args.core_id,
            keys,
            args.allow_private_sources,
            Caps {
                cap: args.cap,
                egress_cap: args.egress_cap,
            },
        )
        .wrap_err_with(|| {
            format!("could not open the store in {}", args.data_dir.display())
        })?,
    );
    if backend_url.is_some() {
        let store = Arc::clone(&store);
        tokio::spawn(async move { store.keys.refresh_daily().await });
    }
    let background = Arc::clone(&store);
    tokio::spawn(async move {
        loop {
            background.collect_garbage().await;
            tokio::time::sleep(GC_EVERY).await;
        }
    });
    let saver = Arc::clone(&store);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            saver.limits.save().await;
        }
    });

    let listener =
        tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let local = local::serve(
        Arc::clone(&store),
        port,
        tokio::io::stdin(),
        tokio::io::stdout(),
    );
    axum::serve(
        listener,
        routes::router(Arc::clone(&store))
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        if let Err(error) = local.await {
            warn!(%error, "lost the pipes to Copal");
        }
    })
    .await?;
    store.limits.save().await;
    Ok(())
}
