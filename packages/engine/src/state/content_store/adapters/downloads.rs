use crate::util::fetch::{
    self, DownloadMeta, DownloadedFile, FetchProgressFn, FetchSemaphore,
};
use sqlx::SqlitePool;
use std::path::Path;

pub(in crate::state::content_store) async fn download(
    staging: &Path,
    pool: &SqlitePool,
    mirrors: &[&str],
    metadata: Option<&DownloadMeta>,
    semaphore: &FetchSemaphore,
    progress: Option<&mut FetchProgressFn<'_>>,
) -> crate::Result<DownloadedFile> {
    fetch::fetch_file_mirrors_in(
        mirrors,
        None,
        metadata,
        None,
        semaphore,
        pool,
        progress,
        Some(staging),
    )
    .await
}
