use std::future::Future;

use contracts::backend::sharing::{
    InstanceVersionId, ReportVersion, VersionFault,
};
use contracts::client::{Remote, call};
use contracts::{Hash, Json};

use super::{FetchError, PoolFile, PoolObject, StoreClient};

/// Fetches one file from the first holder that serves it correctly (see
/// [`StoreClient::fetch_file`]), and reports each holder that served bad bytes to the backend
/// through `backend`, as the reader that read the version. `sink` gets the plain bytes and is
/// called afresh for each holder; it must fail unless it reads the file's full size.
pub async fn fetch<T, E, Fut>(
    store: &StoreClient,
    backend: &Remote<'_>,
    file: &PoolFile,
    downloaded: &(dyn Fn(u64) + Sync),
    sink: impl FnMut(tokio::io::DuplexStream) -> Fut,
) -> Result<T, FetchError<E>>
where
    Fut: Future<Output = Result<T, E>>,
{
    let access = &file.access;
    let object = PoolObject {
        root: Hash(access.root),
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
    let version = InstanceVersionId {
        id: access.share_id.clone(),
        version: access.version,
    };
    for fault in faults {
        let report = Json(VersionFault {
            core_id: fault.core.clone(),
            kind: fault.kind,
        });
        if let Err(error) =
            call::<ReportVersion>(backend, &version, &(), report).await
        {
            tracing::warn!(
                share_id = access.share_id,
                version = access.version,
                core = fault.core,
                kind = ?fault.kind,
                %error,
                "Could not report a shared file failure"
            );
        }
    }
    result
}
