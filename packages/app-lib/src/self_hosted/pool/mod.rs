//! The App's half of the shared file pool (`docs/internals/pool.md`). An owner encrypts each
//! shared file and stores it on a Core; a member fetches it back from one of the version's
//! holders and checks it twice, against C and then P.

pub use sharing_client::{crypto, store};

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ErrorKind;
use store::{FetchError, ListedObject, PoolCore, StoreClient, StoreError};

/// The App's client for Cores' stores, for the life of the process.
static STORE: LazyLock<StoreClient> =
    LazyLock::new(|| StoreClient::new(crate::launcher_user_agent()));

/// What a member needs to fetch one version's files.
#[derive(Debug, Serialize, Deserialize)]
pub struct PoolAccess {
    /// R, in hex.
    pub root: String,
    pub holders: Vec<PoolCore>,
    /// The fetch ticket for `root`.
    pub ticket: String,
    pub shared_instance_id: String,
    pub version: i32,
}

/// One manifest entry that lives in the pool.
#[derive(Debug, Serialize, Deserialize)]
pub struct PoolFile {
    /// C.
    pub object: [u8; 32],
    pub object_size: u64,
    /// Absent for content files, whose key derives from `sha256`.
    pub key: Option<[u8; 32]>,
    /// P.
    pub sha256: [u8; 32],
    /// The access of the version this entry came from.
    pub access: Arc<PoolAccess>,
}

/// A hash written as 64 hex characters.
pub fn parse_hash(text: &str) -> Option<[u8; 32]> {
    let mut hash = [0; 32];
    hex::decode_to_slice(text, &mut hash).ok()?;
    Some(hash)
}

/// Fetches one file from the first holder that serves it correctly (see
/// [`StoreClient::fetch_file`]), and reports the holders that served bad bytes to the backend.
/// `sink` gets the plain bytes and is called afresh for each holder. It must fail unless it reads
/// the file's full size.
pub(crate) async fn fetch_file<T, Fut>(
    file: &PoolFile,
    downloaded: &(dyn Fn(u64) + Sync),
    sink: impl FnMut(tokio::io::DuplexStream) -> Fut,
) -> crate::Result<T>
where
    Fut: Future<Output = crate::Result<T>>,
{
    let access = &file.access;
    let object = store::PoolObject {
        root: &access.root,
        holders: &access.holders,
        ticket: &access.ticket,
        object: file.object,
        object_size: file.object_size,
        plain: file.sha256,
        key: file
            .key
            .unwrap_or_else(|| crypto::content_key(&file.sha256)),
    };
    let mut faults = Vec::new();
    let result = STORE
        .fetch_file(&object, downloaded, &mut faults, sink)
        .await;
    for fault in faults {
        crate::api::instance::report_pool_failure(
            &access.shared_instance_id,
            access.version,
            &fault.core,
            fault.kind.as_str(),
        )
        .await;
    }
    result.map_err(|error| match error {
        FetchError::Broken => ErrorKind::PoolFilesBroken.into(),
        FetchError::Unavailable => ErrorKind::PoolFilesUnavailable.into(),
        FetchError::Sink(error) => error,
    })
}

/// A file encrypted for a push, waiting in the push's folder under its C.
pub(crate) struct PoolObject {
    pub object: [u8; 32],
    pub size: u64,
    /// `None` for a file carried from the previous version, which the Core already holds.
    pub path: Option<PathBuf>,
    /// P of the plaintext that was encrypted, which the push compares to the P it hashed.
    pub plain: [u8; 32],
}

/// Encrypts `source` into `folder`, named by its C once that is known.
pub(crate) async fn encrypt_file(
    key: &[u8; 32],
    source: &Path,
    folder: &Path,
) -> crate::Result<PoolObject> {
    let part = folder.join(format!("{}.part", uuid::Uuid::new_v4()));
    let reader = tokio::fs::File::open(source).await?;
    let writer = tokio::fs::File::create(&part).await?;
    let encrypted = crypto::encrypt(key, reader, writer).await?;
    let path = folder.join(hex::encode(encrypted.object));
    tokio::fs::rename(&part, &path).await?;
    Ok(PoolObject {
        object: encrypted.object,
        size: encrypted.size,
        path: Some(path),
        plain: encrypted.plain,
    })
}

/// Uploads rounds of whatever the Core still lacks until it holds the whole version.
const UPLOAD_ROUNDS: usize = 3;
const UPLOAD_PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// Stores a version's objects on `core`, sending only those it lacks, and returns the C of each
/// object it sent. `on_progress` receives the uploaded and total bytes of those objects.
pub(crate) async fn store_version(
    core: &PoolCore,
    ticket: &str,
    root: &str,
    objects: &[PoolObject],
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
) -> crate::Result<HashSet<String>> {
    let paths = objects
        .iter()
        .filter_map(|object| {
            Some((hex::encode(object.object), object.path.clone()?))
        })
        .collect::<HashMap<_, _>>();
    let mut list = objects
        .iter()
        .map(|object| ListedObject {
            hash: hex::encode(object.object),
            size: object.size,
        })
        .collect::<Vec<_>>();
    list.sort_by(|left, right| left.hash.cmp(&right.hash));
    list.dedup_by(|left, right| left.hash == right.hash);
    let hashes = list
        .iter()
        .map(|object| object.hash.clone())
        .collect::<Vec<_>>();

    let have = STORE
        .have(core, ticket, root, &hashes)
        .await
        .map_err(store_error)?;
    if have.complete {
        return Ok(HashSet::new());
    }
    let mut missing = have.missing;
    if missing.iter().any(|hash| !paths.contains_key(hash)) {
        return Err(ErrorKind::InputError(
            "The Core no longer has a config file shared earlier. Select it under Files and push again."
                .to_string(),
        )
        .into());
    }
    let requested = missing.iter().cloned().collect::<HashSet<_>>();
    let total = list
        .iter()
        .filter(|object| requested.contains(&object.hash))
        .map(|object| object.size)
        .sum();
    let uploaded = Arc::new(AtomicU64::new(0));
    on_progress(0, total);
    for _ in 0..UPLOAD_ROUNDS {
        let sending = missing
            .iter()
            .filter_map(|hash| Some((hash.clone(), paths.get(hash)?.clone())))
            .collect::<Vec<_>>();
        let upload = STORE.upload(
            core,
            ticket,
            root,
            &list,
            sending,
            Arc::clone(&uploaded),
        );
        tokio::pin!(upload);
        let mut ticker = tokio::time::interval(UPLOAD_PROGRESS_INTERVAL);
        let outcome = loop {
            tokio::select! {
                outcome = &mut upload => break outcome,
                _ = ticker.tick() => on_progress(
                    uploaded.load(Ordering::Relaxed).min(total),
                    total,
                ),
            }
        }
        .map_err(store_error)?;
        if outcome.complete {
            on_progress(total, total);
            return Ok(requested);
        }
        missing = outcome.missing;
    }
    Err(anyhow::anyhow!(
        "the Core did not accept every shared file; push again"
    ))
}

fn store_error(error: StoreError) -> crate::Error {
    match error {
        StoreError::DiskFull => ErrorKind::PoolDiskFull.into(),
        StoreError::EgressSpent => ErrorKind::PoolEgressSpent.into(),
        error => anyhow::Error::new(error),
    }
}
