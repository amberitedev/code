//! The App's half of the shared file pool. A member fetches each
//! file back from one of the version's holders with `sharing_client::pool::fetch`, which checks
//! it twice, against the object's hash and then the plain bytes', and reports holders that served bad bytes. An owner's push
//! goes through `sharing_client::push`. Both are the code Core uses too.

pub use sharing_client::pool::{PoolAccess, PoolFile};

use std::future::Future;
use std::sync::LazyLock;

use sharing_client::pool::{FetchError, StoreClient};

use crate::ErrorKind;

/// The App's client for Cores' stores, for the life of the process. Pushes and fetches share
/// it.
pub(crate) static STORE: LazyLock<StoreClient> =
    LazyLock::new(|| StoreClient::new(crate::launcher_user_agent()));

/// Fetches one file from the first holder that serves it correctly, as the signed-in account.
/// `sink` gets the plain bytes and is called afresh for each holder. It must fail unless it reads
/// the file's full size.
pub(crate) async fn fetch_file<T, Fut>(
    sessions: &crate::state::Sessions,
    file: &PoolFile,
    downloaded: &(dyn Fn(u64) + Sync),
    sink: impl FnMut(tokio::io::DuplexStream) -> Fut,
) -> crate::Result<T>
where
    Fut: Future<Output = crate::Result<T>>,
{
    let backend = crate::api::instance::share_backend(sessions)?;
    sharing_client::pool::fetch(
        &STORE,
        &backend.remote(),
        file,
        downloaded,
        sink,
    )
    .await
    .map_err(|error| match error {
        FetchError::Broken => ErrorKind::PoolFilesBroken.into(),
        FetchError::Unavailable => ErrorKind::PoolFilesUnavailable.into(),
        FetchError::Sink(error) => error,
    })
}
