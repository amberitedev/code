//! The App's half of the shared file pool (`docs/internals/pool.md`). A member fetches each
//! file back from one of the version's holders and checks it twice, against C and then P. An
//! owner's push goes through `sharing_client::push`, the writer the App shares with Core.

pub use sharing_client::crypto::parse_hash;
pub use sharing_client::{crypto, store};

use std::future::Future;
use std::sync::{Arc, LazyLock};

use serde::{Deserialize, Serialize};

use crate::ErrorKind;
use store::{FetchError, PoolCore, StoreClient};

/// The App's client for Cores' stores, for the life of the process. Pushes and fetches share
/// it.
pub(crate) static STORE: LazyLock<StoreClient> =
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
