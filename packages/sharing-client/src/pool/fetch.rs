use std::future::Future;

use super::{FetchError, PoolFile, PoolObject, StoreClient};
use crate::backend::Backend;

/// Fetches one file from the first holder that serves it correctly (see
/// [`StoreClient::fetch_file`]), and reports each holder that served bad bytes to the backend
/// through `backend`, as the reader that read the version. `sink` gets the plain bytes and is
/// called afresh for each holder; it must fail unless it reads the file's full size.
pub async fn fetch<T, E, Fut>(
    store: &StoreClient,
    backend: &Backend,
    file: &PoolFile,
    downloaded: &(dyn Fn(u64) + Sync),
    sink: impl FnMut(tokio::io::DuplexStream) -> Fut,
) -> Result<T, FetchError<E>>
where
    Fut: Future<Output = Result<T, E>>,
{
    let access = &file.access;
    let root = hex::encode(access.root);
    let object = PoolObject {
        root: &root,
        holders: &access.holders,
        ticket: &access.ticket,
        object: file.object,
        object_size: file.object_size,
        plain: file.sha256,
        key: file.key,
    };
    let mut faults = Vec::new();
    let result = store
        .fetch_file(&object, downloaded, &mut faults, sink)
        .await;
    // Best effort: the read goes on either way.
    for fault in faults {
        if let Err(error) = backend
            .report_fault(&access.share_id, access.version, &fault)
            .await
        {
            tracing::warn!(
                share_id = access.share_id,
                version = access.version,
                core = fault.core,
                kind = fault.kind.as_str(),
                %error,
                "Could not report a shared file failure"
            );
        }
    }
    result
}
