//! The App's half of the shared file pool (`docs/internals/pool.md`). An owner encrypts each
//! shared file and stores it on a Core; a member fetches it back from one of the version's
//! holders and checks it twice, against C and then P.

pub mod crypto;
pub mod store;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures::TryStreamExt;
use serde::{Deserialize, Serialize};

use crate::ErrorKind;
use crypto::{Decryptor, PoolFetchError};
use store::{ListedObject, PoolCore, StoreError};

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

const PIPE_SIZE: usize = 256 * 1024;
/// Further tries at one holder after a dropped connection, each from where the last stopped.
const RESUME_ATTEMPTS: usize = 2;
/// Waits at one holder that answered it is busy (its fetch slots or the per-address rate), with
/// the wait doubling up to [`RATE_LIMIT_MAX_WAIT`]: about three minutes in all, which covers the
/// store's one-minute rate window during a large install.
const RATE_LIMIT_WAITS: u32 = 8;
const RATE_LIMIT_MAX_WAIT: Duration = Duration::from_secs(30);

/// Fetches one file from the first holder that serves it correctly. `sink` gets the plain bytes
/// and is called afresh for each holder. It must fail unless it reads the file's full size: the
/// last chunk is only released once the whole file checks out, so a sink that requires the size
/// never completes with a wrong file.
pub(crate) async fn fetch_file<T, Fut>(
    file: &PoolFile,
    downloaded: &(dyn Fn(u64) + Sync),
    mut sink: impl FnMut(tokio::io::DuplexStream) -> Fut,
) -> crate::Result<T>
where
    Fut: Future<Output = crate::Result<T>>,
{
    let access = &file.access;
    let key = file
        .key
        .unwrap_or_else(|| crypto::content_key(&file.sha256));
    let object = hex::encode(file.object);
    for core in &access.holders {
        let (writer, reader) = tokio::io::duplex(PIPE_SIZE);
        let decrypt = async {
            let mut writer = writer;
            let mut decryptor = Decryptor::new(&key, file.object, file.sha256);
            let mut attempt = 0;
            let mut waits = 0;
            loop {
                match fetch_once(
                    core,
                    access,
                    &object,
                    file.object_size,
                    &mut decryptor,
                    &mut writer,
                    downloaded,
                )
                .await
                {
                    Err(PoolFetchError::Http(StoreError::RateLimited))
                        if waits < RATE_LIMIT_WAITS =>
                    {
                        tokio::time::sleep(
                            Duration::from_secs(1 << waits)
                                .min(RATE_LIMIT_MAX_WAIT),
                        )
                        .await;
                        waits += 1;
                    }
                    Err(error)
                        if attempt < RESUME_ATTEMPTS
                            && is_transient(&error) =>
                    {
                        attempt += 1;
                    }
                    result => break result,
                }
            }
        };
        let (decrypted, sunk) = tokio::join!(decrypt, sink(reader));
        match decrypted {
            Ok(()) => return sunk,
            // The sink stopped reading on its own; its error is the real one.
            Err(PoolFetchError::Io(error))
                if error.kind() == std::io::ErrorKind::BrokenPipe =>
            {
                return sunk;
            }
            Err(PoolFetchError::WrongContent) => {
                // Every holder has the same bytes, so trying another cannot help.
                crate::api::instance::report_pool_failure(
                    &access.shared_instance_id,
                    access.version,
                    &core.id,
                    "content",
                )
                .await;
                return Err(ErrorKind::PoolFilesBroken.into());
            }
            Err(error) => {
                tracing::warn!(
                    core = %core.id,
                    object,
                    error = %error,
                    "Could not fetch a shared file from this Core"
                );
                if matches!(error, PoolFetchError::WrongObject) {
                    crate::api::instance::report_pool_failure(
                        &access.shared_instance_id,
                        access.version,
                        &core.id,
                        "object",
                    )
                    .await;
                }
            }
        }
    }
    Err(ErrorKind::PoolFilesUnavailable.into())
}

fn is_transient(error: &PoolFetchError) -> bool {
    match error {
        PoolFetchError::Io(error) => {
            error.kind() != std::io::ErrorKind::BrokenPipe
        }
        PoolFetchError::Http(StoreError::Http(_)) => true,
        _ => false,
    }
}

async fn fetch_once(
    core: &PoolCore,
    access: &PoolAccess,
    object: &str,
    object_size: u64,
    decryptor: &mut Decryptor,
    writer: &mut tokio::io::DuplexStream,
    downloaded: &(dyn Fn(u64) + Sync),
) -> Result<(), PoolFetchError> {
    let from = decryptor.offset();
    let response =
        store::fetch(core, &access.ticket, &access.root, object, from).await?;
    if response
        .content_length()
        .is_some_and(|length| length != object_size.saturating_sub(from))
    {
        return Err(PoolFetchError::WrongObject);
    }
    let body = tokio_util::io::StreamReader::new(
        response
            .bytes_stream()
            .map_err(std::io::Error::other)
            .inspect_ok(|chunk| downloaded(chunk.len() as u64)),
    );
    decryptor.run(body, writer).await
}

/// A file encrypted for a push, waiting in the push's folder under its C.
pub(crate) struct PoolObject {
    pub object: [u8; 32],
    pub size: u64,
    pub path: PathBuf,
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
        path,
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
        .map(|object| (hex::encode(object.object), object.path.clone()))
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

    let have = store::have(core, ticket, root, &hashes)
        .await
        .map_err(store_error)?;
    if have.complete {
        return Ok(HashSet::new());
    }
    let mut missing = have.missing;
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
        let upload = store::upload(
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
