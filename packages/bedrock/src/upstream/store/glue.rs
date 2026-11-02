//! The glue for the content store clone (`ports.toml`): everything the cloned files use from
//! app-lib, so they compile unchanged without app-lib's `State`.
//!
//! The clone is mounted at `crate::state::content_store`, the path upstream's files name it by,
//! and these items are re-exported where the files look for them (`crate::util`,
//! `crate::state`, `crate::install`, `crate::State`). The host is not the App: it has no
//! instances, no runtime cache and no install jobs, so the stand-ins for those report none, and
//! the store keeps only content its host retains.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use anyhow::{Context, bail};
use futures::StreamExt;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::io::AsyncWriteExt;

use crate::state::content_store::StoredFileHandle;
pub use crate::state::content_store::{ContentStore, StoreUsage};

/// The tables the store reads and writes, for a host's own migration. `instance_files` holds
/// only the columns the store reads; a host without instances keeps it empty.
pub const SCHEMA: &str = include_str!("schema.sql");

/// What the cloned commands take where upstream takes app-lib's `State`.
pub struct State {
    pub pool: SqlitePool,
    pub(crate) fetch_semaphore: fetch::FetchSemaphore,
}

impl State {
    /// The state for a store on `pool`, downloading up to 8 files at once.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            fetch_semaphore: fetch::FetchSemaphore(
                tokio::sync::Semaphore::new(8),
            ),
        }
    }
}

/// The folders the store lives in, as app-lib's `DirectoryInfo` names them.
pub struct DirectoryInfo {
    /// Where the store's process lock is.
    pub settings_dir: PathBuf,
    /// The root of `store/` and `profiles/`.
    pub config_dir: PathBuf,
}

impl DirectoryInfo {
    pub fn store_dir(&self) -> PathBuf {
        self.config_dir.join("store")
    }

    pub fn content_store_dir(&self) -> PathBuf {
        self.store_dir().join("content")
    }

    pub fn store_staging_dir(&self) -> PathBuf {
        self.store_dir().join("staging")
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.config_dir.join("profiles")
    }
}

/// Open the store under `dirs` on `pool`, whose database has [`SCHEMA`], and recover what an
/// interrupted run left.
pub async fn open(
    dirs: &DirectoryInfo,
    pool: SqlitePool,
) -> crate::Result<ContentStore> {
    let lock = ContentStore::lock_process(&dirs.settings_dir).await?;
    let store = ContentStore::new(dirs, pool.clone(), lock).await?;
    let state = State::new(pool);
    crate::state::content_store::migrate(&store, &state).await?;
    Ok(store)
}

/// A blob in the store, by its SHA-512. Cleanup cannot remove it while this is held, so read
/// [`Stored::path`] before dropping it.
pub struct Stored(StoredFileHandle);

impl Stored {
    pub fn sha512(&self) -> &str {
        &self.0.metadata.sha512
    }

    pub fn size(&self) -> u64 {
        self.0.metadata.size as u64
    }

    pub fn path(&self) -> &Path {
        &self.0.path
    }
}

/// Put `bytes` in the store. Unretained content is removed by the next cleanup, so
/// [`retain`] what must be kept.
pub async fn store_bytes(
    store: &ContentStore,
    bytes: &[u8],
) -> crate::Result<Stored> {
    store.store_bytes(bytes).await.map(Stored)
}

/// Put a copy of the file at `path` in the store. Unretained content is removed by the next
/// cleanup, so [`retain`] what must be kept.
pub async fn store_file(
    store: &ContentStore,
    path: &Path,
) -> crate::Result<Stored> {
    store.store_file(path).await.map(Stored)
}

/// The healthy blob with `sha512`, if the store has it.
pub async fn lookup(
    store: &ContentStore,
    sha512: &str,
) -> crate::Result<Option<Stored>> {
    Ok(store.lookup(Some(sha512), None).await?.map(Stored))
}

/// Keep `sha512s` through cleanup for `owner`, a key of the host's choosing within `kind`
/// (for example a share's history).
pub async fn retain(
    store: &ContentStore,
    kind: &str,
    owner: &str,
    sha512s: &[String],
) -> crate::Result<()> {
    store.retain(kind, owner, sha512s).await
}

/// Drop everything `owner` retains within `kind`; cleanup may then remove it.
pub async fn release(
    store: &ContentStore,
    kind: &str,
    owner: &str,
) -> crate::Result<()> {
    store.release(kind, owner).await
}

/// Remove content nothing retains and nothing has used for a minute, as upstream's periodic
/// cleanup does with its cache limit at zero: the host keeps only what it retains. Returns the
/// bytes freed. Fails while other store work holds the store; try again later.
pub async fn cleanup(
    store: &ContentStore,
    pool: SqlitePool,
) -> crate::Result<u64> {
    store.set_cache_limit(0).await?;
    store.cleanup(&State::new(pool), false).await
}

/// app-lib's `file_modified_at_ns` (`state/cache.rs`).
pub(crate) fn file_modified_at_ns(
    metadata: &std::fs::Metadata,
) -> std::io::Result<u64> {
    let elapsed = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(std::io::Error::other)?;
    Ok(elapsed
        .as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(elapsed.subsec_nanos())))
}

/// An instance, as far as the store reads one.
pub(crate) struct Instance {
    pub id: String,
    pub path: String,
}

/// A Modrinth version, as far as the store repairs a file from its sources.
#[derive(Deserialize)]
pub(crate) struct Version {
    pub files: Vec<VersionFile>,
}

#[derive(Deserialize)]
pub(crate) struct VersionFile {
    pub hashes: HashMap<String, String>,
    pub url: String,
}

/// Instances run nowhere: the host has none.
pub(crate) async fn instance_has_running_process(
    _instance_id: &str,
    _state: &State,
) -> crate::Result<bool> {
    Ok(false)
}

/// The instance rows the store reads. The host has no instances.
pub(crate) mod instances {
    use sqlx::{Sqlite, Transaction};

    use super::Instance;
    use crate::state::InstanceFile;

    pub(crate) async fn load_instance_rows(
        _pool: &sqlx::SqlitePool,
    ) -> crate::Result<Vec<Instance>> {
        Ok(Vec::new())
    }

    pub(crate) async fn get_instance_files<'e, E>(
        _instance_id: &str,
        _exec: E,
    ) -> crate::Result<Vec<InstanceFile>>
    where
        E: sqlx::Executor<'e, Database = Sqlite>,
    {
        Ok(Vec::new())
    }

    pub(crate) async fn get_instance_file_by_relative_path(
        _instance_id: &str,
        _relative_path: &str,
        _pool: &sqlx::SqlitePool,
    ) -> crate::Result<Option<InstanceFile>> {
        Ok(None)
    }

    pub(crate) async fn upsert_instance_file(
        _file: &InstanceFile,
        _tx: &mut Transaction<'_, Sqlite>,
    ) -> crate::Result<InstanceFile> {
        anyhow::bail!(crate::ErrorKind::ContentStoreError(
            "this host has no instance files".into()
        ))
    }
}

/// app-lib's runtime cache (game and Java files): the host keeps none.
pub(crate) mod runtime_cache {
    use std::path::{Path, PathBuf};

    use super::State;

    pub(crate) struct RuntimeFile {
        pub last_used_at: i64,
        pub references: usize,
    }

    pub(crate) struct RuntimeStorage {
        pub root: PathBuf,
        pub files: Vec<RuntimeFile>,
    }

    impl RuntimeStorage {
        pub async fn read(_state: &State) -> crate::Result<Self> {
            Ok(Self {
                root: PathBuf::new(),
                files: Vec::new(),
            })
        }

        pub fn total_bytes(&self) -> u64 {
            0
        }

        pub fn shared_bytes(&self) -> u64 {
            0
        }

        pub fn unused_bytes(&self) -> u64 {
            0
        }

        pub fn saved_bytes(&self) -> u64 {
            0
        }
    }

    pub(crate) async fn remove_file(
        _file: &RuntimeFile,
        _root: &Path,
    ) -> crate::Result<u64> {
        Ok(0)
    }
}

/// app-lib's install jobs, whose backups hold store content: the host runs none.
#[allow(dead_code)]
pub(crate) mod install {
    use std::path::PathBuf;

    pub(crate) enum InstallJobStatus {
        Queued,
        Running,
    }

    pub(crate) struct InstallJob {
        pub status: InstallJobStatus,
        pub state: InstallJobState,
    }

    pub(crate) struct InstallJobState {
        pub paths: InstallJobPaths,
    }

    pub(crate) struct InstallJobPaths {
        pub staging_dir: Option<PathBuf>,
    }

    pub(crate) mod store {
        use super::InstallJob;

        pub(crate) async fn list(
            _include_finished: bool,
            _pool: &sqlx::SqlitePool,
        ) -> crate::Result<Vec<InstallJob>> {
            Ok(Vec::new())
        }
    }
}

/// app-lib's downloads (`util/fetch.rs`), as far as the store downloads: a file from the first
/// mirror that serves it, staged and hashed, and a small JSON document.
pub(crate) mod fetch {
    use std::future::Future;
    use std::pin::Pin;

    use super::*;

    pub use tokio::sync::Semaphore;

    /// Downloads at once.
    pub struct FetchSemaphore(pub Semaphore);

    /// The headers app-lib sends Modrinth about a download. The host sends none.
    pub struct DownloadMeta;

    pub type FetchProgressFn<'a> = dyn FnMut(
            u64,
            u64,
        )
            -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + 'a>>
        + Send
        + 'a;

    pub(crate) struct DownloadedFile {
        path: Arc<tempfile::TempPath>,
        pub size: u64,
        pub sha512: String,
    }

    impl DownloadedFile {
        pub(crate) fn into_staged(self) -> crate::Result<StagedDownload> {
            let path = Arc::try_unwrap(self.path).map_err(|_| {
                crate::ErrorKind::InputError(
                    "downloaded content is still in use and cannot be published"
                        .to_string(),
                )
            })?;
            Ok(StagedDownload {
                path,
                size: self.size,
                sha512: self.sha512,
            })
        }
    }

    #[derive(Debug)]
    pub(crate) struct StagedDownload {
        pub(crate) path: tempfile::TempPath,
        pub(crate) size: u64,
        pub(crate) sha512: String,
    }

    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::Client::builder()
            .user_agent(concat!("amberite/bedrock/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(15))
            .read_timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("the download client builds")
    });

    /// Download from the first of `mirrors` that serves the file, into `staging`. `sha1`,
    /// `download_meta` and `uri_path` are app-lib's and unused here: the store checks SHA-512.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn fetch_file_mirrors_in(
        mirrors: &[&str],
        _sha1: Option<&str>,
        _download_meta: Option<&DownloadMeta>,
        _uri_path: Option<&'static str>,
        semaphore: &FetchSemaphore,
        _pool: &SqlitePool,
        mut progress: Option<&mut FetchProgressFn<'_>>,
        staging: Option<&Path>,
    ) -> crate::Result<DownloadedFile> {
        if mirrors.is_empty() {
            bail!(crate::ErrorKind::InputError(
                "no mirrors provided".to_string()
            ));
        }
        let _permit = semaphore.0.acquire().await?;
        let mut last = None;
        for mirror in mirrors {
            match download(mirror, progress.as_deref_mut(), staging).await {
                Ok(file) => return Ok(file),
                Err(error) => last = Some(error),
            }
        }
        Err(last.expect("at least one mirror was tried"))
    }

    async fn download(
        url: &str,
        mut progress: Option<&mut FetchProgressFn<'_>>,
        staging: Option<&Path>,
    ) -> crate::Result<DownloadedFile> {
        let response = CLIENT
            .get(url)
            .send()
            .await?
            .error_for_status()
            .with_context(|| format!("could not download {url}"))?;
        let total = response.content_length().unwrap_or(0);
        let (mut file, path) =
            crate::util::content_hash::temporary_file(staging).await?;
        let mut hasher = crate::util::content_hash::ContentHasher::default();
        let mut size = 0_u64;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            hasher.update(&chunk);
            size += chunk.len() as u64;
            if let Some(progress) = progress.as_mut() {
                progress(size, total).await?;
            }
        }
        file.sync_all().await?;
        drop(file);
        Ok(DownloadedFile {
            path: Arc::new(path),
            size,
            sha512: hasher.finish(size).sha512,
        })
    }

    /// GET `url` and read its JSON. The body, header and route arguments are app-lib's and
    /// unused here.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn fetch_json<T: serde::de::DeserializeOwned>(
        method: reqwest::Method,
        url: &str,
        _json_body: Option<serde_json::Value>,
        _header: Option<(&str, &str)>,
        _uri_path: Option<&'static str>,
        semaphore: &FetchSemaphore,
        _pool: &SqlitePool,
    ) -> crate::Result<T> {
        let _permit = semaphore.0.acquire().await?;
        Ok(CLIENT
            .request(method, url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}
