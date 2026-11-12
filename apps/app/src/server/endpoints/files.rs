//! Files on this computer, for an `<img>` or `<video>`. The secret path segment is their only
//! credential, since such a load sends no headers, and any refusal is a bare 404.

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
