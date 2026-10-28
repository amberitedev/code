//! The shared file pool's store (`docs/internals/pool.md`): encrypted objects named by hash, kept
//! for versions a backend-signed ticket names, served to whoever holds a fetch ticket.
//!
//! It is `copal store`, a second process Core starts and supervises (see [`supervisor`]), so it
//! holds no database, no session, and neither Core's backend key nor its TLS key: a compromise
//! of it yields no identity. It listens on `127.0.0.1` only; Core forwards `/store/...` to it
//! unchanged and calls `/admin/...` with a token it hands the store at start.

mod limits;
mod objects;
pub(crate) mod pull;
mod routes;
pub mod supervisor;
mod tar;
mod tickets;

use std::{
    io::Write,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use color_eyre::eyre::{Result, WrapErr, bail};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{access::keys::Keys, api::backend::HeldVersion};
use limits::Limits;
use objects::{Hash, Index, Layout, ListState, VersionList};
pub use supervisor::Storage;

/// The environment variable the admin token comes in, so it never shows in a process list.
pub const ADMIN_TOKEN_ENV: &str = "COPAL_STORE_ADMIN_TOKEN";
/// Set to `1` to let a pull copy from a private address, as Cores do in development.
pub const ALLOW_PRIVATE_ENV: &str = "COPAL_STORE_ALLOW_PRIVATE_SOURCES";
/// Room the disk keeps after an upload, so the store never fills the disk the Core uses.
const DISK_RESERVE: u64 = 256 * 1024 * 1024;
const GC_EVERY: Duration = Duration::from_secs(10 * 60);

/// `copal store`, as the Core starts it.
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
}

/// The store's caps, as `/admin/config` takes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caps {
    pub cap: u64,
    pub egress_cap: u64,
}

/// `GET /admin/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminStatus {
    pub cap: u64,
    /// Bytes the stored, pending, and dropped lists name.
    pub used: u64,
    /// Free bytes on the store's disk.
    pub free: u64,
    pub egress_cap: u64,
    pub egress_today: u64,
    pub versions: Vec<HeldVersion>,
}

/// `POST /admin/drop`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drop {
    pub roots: Vec<String>,
}

/// An error code of the store, as `{ error, description }` like Core's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    TicketInvalid,
    TicketExpired,
    TicketWrongCore,
    TicketWrongKind,
    NotFound,
    DiskFull,
    EgressSpent,
    RateLimited,
    InvalidRequest,
    Conflict,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreError {
    pub error: Code,
    pub description: String,
}

impl StoreError {
    pub fn new(error: Code, description: impl Into<String>) -> Self {
        Self {
            error,
            description: description.into(),
        }
    }
}

impl IntoResponse for StoreError {
    fn into_response(self) -> Response {
        let status = match self.error {
            Code::TicketInvalid
            | Code::TicketExpired
            | Code::TicketWrongCore
            | Code::TicketWrongKind => StatusCode::UNAUTHORIZED,
            Code::NotFound => StatusCode::NOT_FOUND,
            Code::DiskFull => StatusCode::INSUFFICIENT_STORAGE,
            Code::EgressSpent | Code::RateLimited => {
                StatusCode::TOO_MANY_REQUESTS
            }
            Code::InvalidRequest => StatusCode::BAD_REQUEST,
            Code::Conflict => StatusCode::CONFLICT,
            Code::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        warn!(%error, "the store failed");
        Self::new(Code::Internal, "The store failed. Its log says why.")
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Everything the store's routes reach.
pub struct Store {
    layout: Layout,
    core_id: String,
    keys: Keys,
    admin_token: String,
    allow_private_sources: bool,
    caps: Mutex<Caps>,
    index: Mutex<Index>,
    limits: Limits,
}

impl Store {
    /// The store in `layout`'s folder, creating it when missing.
    fn open(
        layout: Layout,
        core_id: String,
        keys: Keys,
        admin_token: String,
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
            admin_token,
            allow_private_sources,
            caps: Mutex::new(caps),
            index: Mutex::new(index),
        })
    }

    fn caps(&self) -> Caps {
        *self.caps.lock().expect("not poisoned")
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
    ) -> Result<Vec<Hash>, StoreError> {
        let invalid =
            |description| StoreError::new(Code::InvalidRequest, description);
        list.check(root).map_err(invalid)?;
        if max_bytes.is_some_and(|max| list.bytes() > max) {
            return Err(invalid(
                "The version is larger than its ticket allows.",
            ));
        }
        let missing = objects::missing(&self.layout, &list).await?;
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
                return Err(StoreError::new(
                    Code::DiskFull,
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
        let free = crate::backups::space::free(self.layout.root())
            .inspect_err(|_| unreserve())?;
        if free < needed + DISK_RESERVE {
            unreserve();
            return Err(StoreError::new(
                Code::DiskFull,
                "This Core's disk is full.",
            ));
        }
        if let Some(at) = reserved {
            objects::write_atomic(
                &self.layout.list(root, ListState::Pending(at)),
                &serde_json::to_vec(list.as_ref()).expect("lists serialize"),
            )
            .await
            .inspect_err(|_| unreserve())?;
        }
        Ok(missing)
    }

    /// Store `root`'s pending list once every object is present. Returns those still missing.
    async fn finish(&self, root: &Hash) -> Result<Vec<Hash>, StoreError> {
        let pending = self.index().pending.get(root).cloned();
        let Some((at, list)) = pending else {
            if self.is_stored(root) {
                return Ok(Vec::new());
            }
            return Err(StoreError::new(
                Code::Conflict,
                "This upload was abandoned. Start it again.",
            ));
        };
        let missing = objects::missing(&self.layout, &list).await?;
        if !missing.is_empty() {
            return Ok(missing);
        }
        objects::write_atomic(
            &self.layout.list(root, ListState::Stored),
            &serde_json::to_vec(list.as_ref()).expect("lists serialize"),
        )
        .await?;
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

    fn status(&self) -> std::io::Result<AdminStatus> {
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
        Ok(AdminStatus {
            cap: caps.cap,
            used,
            free: crate::backups::space::free(self.layout.root())?,
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

/// `copal store`: serve until stdin closes, which is how Core stops it.
pub async fn main(args: Args) -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "copal=info".into()),
        )
        // Core writes each line into its own log, which adds the time.
        .without_time()
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .try_init();
    let admin_token = std::env::var(ADMIN_TOKEN_ENV).unwrap_or_default();
    if admin_token.len() < 32 {
        bail!("{ADMIN_TOKEN_ENV} must hold the admin token Core made");
    }
    let backend_url =
        Some(args.backend_url).filter(|url| !url.is_empty() && url != "none");
    let http = reqwest::Client::builder()
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
            admin_token,
            std::env::var(ALLOW_PRIVATE_ENV).is_ok_and(|value| value == "1"),
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
    // The one line Core waits for. Everything else goes to stderr.
    let mut stdout = std::io::stdout();
    writeln!(stdout, "listening {port}")?;
    stdout.flush()?;

    let (closed, stdin_closed) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
        let _ = closed.send(());
    });
    axum::serve(
        listener,
        routes::router(Arc::clone(&store))
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = stdin_closed.await;
    })
    .await?;
    store.limits.save().await;
    Ok(())
}
