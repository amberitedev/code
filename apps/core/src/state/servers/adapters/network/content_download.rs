//! Downloading content (pack files, `.mrpack`s, addons) to a path on a server's disk: checked
//! against its hash, and cut off once it no longer fits in the disk's free space less what Core
//! keeps free. Core's own fixed sources (loaders, Java) use `installs::download` with their own
//! caps.

use std::path::Path;

use engine::archive::{self, NoRoom};

use crate::{
    installs::download::{self, DownloadError, FileDownload, Hash, Progress},
    state::servers::Error,
};

/// Download `url` into `path` with `http`, the client for allowed hosts. The room is read when
/// the download starts, so downloads that run at once each count what the others have not
/// written yet; the whole install was checked against the disk before any of them started.
pub async fn fetch(
    http: &reqwest::Client,
    url: &str,
    path: &Path,
    hash: Hash<'_>,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<(), Error> {
    let parent = path.parent().unwrap_or(path);
    tokio::fs::create_dir_all(parent).await?;
    let room = archive::room(parent)?;
    download::file(
        http,
        url,
        path,
        &FileDownload {
            hash,
            limit: Some(room),
            progress,
        },
    )
    .await
    .map_err(|error| match error {
        DownloadError::TooLarge { url, .. } => NoRoom {
            what: url,
            needed: None,
            room,
        }
        .into(),
        error => error.into(),
    })
}
