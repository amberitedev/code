//! Files on this computer. An `<img>` or `<video>` load sends no headers, so for `/app/file` and
//! `/app/thumbnail` the secret path segment is the only credential, and any refusal is a bare 404.

use std::path::PathBuf;

use contracts::{Empty, Json, Stream};
use serde::{Deserialize, Serialize};

contracts::endpoints! {
    group localFiles;

    /// Serves the file at `path`.
    GetLocalFile: Get "/app/file", None, (), LocalFileQuery, Empty => Stream;
    /// Serves the image at `path` scaled to fit `size` pixels, or the image itself when it cannot be
    /// scaled.
    GetThumbnail: Get "/app/thumbnail", None, (), ThumbnailQuery, Empty => Stream;
    /// Downloads an `https://` icon into the icon cache and returns its path there.
    CacheRemoteIcon: Post "/app/icons/remote", None, (), (), Json<RemoteIcon> => Json<PathBuf>;
    /// The bytes of the file at `path`, for a page that reads a picked or dropped file.
    ReadLocalFile: Get "/app/fs/read", None, (), LocalFileQuery, Empty => Stream;
    /// The name and size of the file at `path`.
    StatLocalFile: Get "/app/fs/stat", None, (), LocalFileQuery, Empty => Json<FileStat>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LocalFileQuery {
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ThumbnailQuery {
    pub path: PathBuf,
    pub size: u32,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct RemoteIcon {
    pub source: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct FileStat {
    pub name: String,
    pub size: u64,
}
