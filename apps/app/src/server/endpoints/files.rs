//! Files on this computer. An `<img>` or `<video>` load sends no headers, so for `/app/file` and
//! `/app/thumbnail` the secret path segment is the only credential, and any refusal is a bare 404.

use std::path::PathBuf;

use contracts::{Auth, Empty, Endpoint, Json, Method, Stream};
use serde::{Deserialize, Serialize};

/// Serves the file at `path`.
pub struct GetLocalFile;

impl Endpoint for GetLocalFile {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/file";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = LocalFileQuery;
    type Body = Empty;
    type Response = Stream;
}

/// Serves the image at `path` scaled to fit `size` pixels, or the image itself when it cannot be
/// scaled.
pub struct GetThumbnail;

impl Endpoint for GetThumbnail {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/thumbnail";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ThumbnailQuery;
    type Body = Empty;
    type Response = Stream;
}

/// Downloads an `https://` icon into the icon cache and returns its path there.
pub struct CacheRemoteIcon;

impl Endpoint for CacheRemoteIcon {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/icons/remote";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<RemoteIcon>;
    type Response = Json<PathBuf>;
}

/// The bytes of the file at `path`, for a page that reads a picked or dropped file.
pub struct ReadLocalFile;

impl Endpoint for ReadLocalFile {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/fs/read";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = LocalFileQuery;
    type Body = Empty;
    type Response = Stream;
}

/// The name and size of the file at `path`.
pub struct StatLocalFile;

impl Endpoint for StatLocalFile {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/fs/stat";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = LocalFileQuery;
    type Body = Empty;
    type Response = Json<FileStat>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LocalFileQuery {
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ThumbnailQuery {
    pub path: PathBuf,
    pub size: u32,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RemoteIcon {
    pub source: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FileStat {
    pub name: String,
    pub size: u64,
}
