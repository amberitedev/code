//! Every download Core makes. A request that stalls fails instead of hanging, failures that can
//! pass are retried, a file is checked against the hash its source publishes, and it lands whole:
//! it is written beside its destination and renamed into place, so a reader never sees half of
//! one and a file a running server uses is never truncated.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use bytes::Bytes;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use sha1::Digest;
use tokio::io::AsyncWriteExt;

/// Tries per download, the first included.
const ATTEMPTS: u32 = 3;
/// The longest a metadata request may take, start to end.
const METADATA_TIMEOUT: Duration = Duration::from_secs(60);
/// A connection that sends nothing for this long is dropped.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("{url}: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{url} answered {status}")]
    Status { url: String, status: StatusCode },
    #[error("{url} sent a file that does not match its hash")]
    Hash { url: String },
    #[error("{url} is larger than {limit} bytes")]
    TooLarge { url: String, limit: u64 },
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl DownloadError {
    /// Whether trying again can help.
    fn transient(&self) -> bool {
        match self {
            Self::Http { .. } | Self::Hash { .. } => true,
            Self::Status { status, .. } => {
                status.is_server_error()
                    || *status == StatusCode::TOO_MANY_REQUESTS
            }
            Self::TooLarge { .. } | Self::Io(_) => false,
        }
    }
}

/// The hash a source publishes for a file, hex encoded.
#[derive(Debug, Clone, Copy)]
pub enum Hash<'a> {
    Sha1(&'a str),
    Sha256(&'a str),
    Sha512(&'a str),
    /// Some library mirrors publish none.
    Unknown,
}

enum Hasher {
    Sha1(sha1::Sha1),
    Sha256(sha2::Sha256),
    Sha512(sha2::Sha512),
    None,
}

impl Hasher {
    fn new(hash: Hash) -> Self {
        match hash {
            Hash::Sha1(_) => Self::Sha1(sha1::Sha1::new()),
            Hash::Sha256(_) => Self::Sha256(sha2::Sha256::new()),
            Hash::Sha512(_) => Self::Sha512(sha2::Sha512::new()),
            Hash::Unknown => Self::None,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Sha1(hasher) => hasher.update(bytes),
            Self::Sha256(hasher) => hasher.update(bytes),
            Self::Sha512(hasher) => hasher.update(bytes),
            Self::None => {}
        }
    }

    fn matches(self, hash: Hash) -> bool {
        let actual = match self {
            Self::Sha1(hasher) => hex::encode(hasher.finalize()),
            Self::Sha256(hasher) => hex::encode(hasher.finalize()),
            Self::Sha512(hasher) => hex::encode(hasher.finalize()),
            Self::None => return true,
        };
        match hash {
            Hash::Sha1(expected)
            | Hash::Sha256(expected)
            | Hash::Sha512(expected) => {
                expected.is_empty() || actual.eq_ignore_ascii_case(expected)
            }
            Hash::Unknown => true,
        }
    }
}

/// The client for Core's own fixed sources: Modrinth's API and launcher metadata, Mojang, PaperMC,
/// the Forge and NeoForge mavens, and Azul. Pack content uses `content::hosts` instead.
pub fn client() -> reqwest::Result<reqwest::Client> {
    builder().build()
}

/// The timeouts and user agent of every Core client.
pub fn builder() -> reqwest::ClientBuilder {
    crate::network::http()
        .user_agent(concat!(
            "amberite/copal/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/amberitedev/code)"
        ))
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
}

/// Run `attempt` up to [`ATTEMPTS`] times while it fails in a way that can pass.
async fn retried<T, F, Fut>(mut attempt: F) -> Result<T, DownloadError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, DownloadError>>,
{
    let mut tries = 0;
    loop {
        tries += 1;
        match attempt().await {
            Err(error) if tries < ATTEMPTS && error.transient() => {
                tracing::debug!(%error, tries, "download failed; trying again");
                tokio::time::sleep(Duration::from_secs(u64::from(
                    tries * tries,
                )))
                .await;
            }
            result => return result,
        }
    }
}

async fn get(
    http: &reqwest::Client,
    url: &str,
    timeout: Option<Duration>,
) -> Result<reqwest::Response, DownloadError> {
    let http_error = |source| DownloadError::Http {
        url: url.to_owned(),
        source,
    };
    let mut request = http.get(url);
    if let Some(timeout) = timeout {
        request = request.timeout(timeout);
    }
    let response = request.send().await.map_err(http_error)?;
    let status = response.status();
    if !status.is_success() {
        return Err(DownloadError::Status {
            url: url.to_owned(),
            status,
        });
    }
    Ok(response)
}

/// A small document, such as a version manifest.
pub async fn bytes(
    http: &reqwest::Client,
    url: &str,
) -> Result<Bytes, DownloadError> {
    retried(|| async {
        get(http, url, Some(METADATA_TIMEOUT))
            .await?
            .bytes()
            .await
            .map_err(|source| DownloadError::Http {
                url: url.to_owned(),
                source,
            })
    })
    .await
}

/// A JSON document. A body that does not parse is not retried.
pub async fn json<T: DeserializeOwned>(
    http: &reqwest::Client,
    url: &str,
) -> Result<T, DownloadError> {
    let bytes = bytes(http, url).await?;
    serde_json::from_slice(&bytes).map_err(|error| {
        DownloadError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{url}: {error}"),
        ))
    })
}

/// What a file download is allowed and told.
pub struct FileDownload<'a> {
    pub hash: Hash<'a>,
    /// Refuse a file larger than this.
    pub limit: Option<u64>,
    /// Told every chunk's size as it arrives. A retry starts over and says so with a reset.
    pub progress: &'a (dyn Fn(Progress) + Sync),
}

#[derive(Debug, Clone, Copy)]
pub enum Progress {
    /// `bytes` more arrived; `total` is the file's size when the source gives it.
    Bytes { bytes: u64, total: Option<u64> },
    /// The download starts over.
    Reset,
}

/// No progress to report.
pub fn quiet(_: Progress) {}

/// Download `url` into `path`, replacing any file there only once the new one is whole and
/// checked. Parent folders are created.
pub async fn file(
    http: &reqwest::Client,
    url: &str,
    path: &Path,
    download: &FileDownload<'_>,
) -> Result<(), DownloadError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut first = true;
    retried(|| {
        if !std::mem::take(&mut first) {
            (download.progress)(Progress::Reset);
        }
        attempt_file(http, url, path, download)
    })
    .await
}

async fn attempt_file(
    http: &reqwest::Client,
    url: &str,
    path: &Path,
    download: &FileDownload<'_>,
) -> Result<(), DownloadError> {
    let response = get(http, url, None).await?;
    let total = response.content_length();
    let too_large = || DownloadError::TooLarge {
        url: url.to_owned(),
        limit: download.limit.unwrap_or_default(),
    };
    if let (Some(limit), Some(total)) = (download.limit, total)
        && total > limit
    {
        return Err(too_large());
    }
    let staged = staging_path(path);
    let result = async {
        let mut file = tokio::fs::File::create(&staged).await?;
        let mut hasher = Hasher::new(download.hash);
        let mut written = 0u64;
        let mut response = response;
        while let Some(chunk) =
            response
                .chunk()
                .await
                .map_err(|source| DownloadError::Http {
                    url: url.to_owned(),
                    source,
                })?
        {
            written += chunk.len() as u64;
            if download.limit.is_some_and(|limit| written > limit) {
                return Err(too_large());
            }
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            (download.progress)(Progress::Bytes {
                bytes: chunk.len() as u64,
                total,
            });
        }
        file.flush().await?;
        drop(file);
        if !hasher.matches(download.hash) {
            return Err(DownloadError::Hash {
                url: url.to_owned(),
            });
        }
        tokio::fs::rename(&staged, path).await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&staged).await;
    }
    result
}

/// Write `bytes` to `path` whole, through a file beside it.
pub async fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let staged = staging_path(path);
    let result = async {
        tokio::fs::write(&staged, bytes).await?;
        tokio::fs::rename(&staged, path).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&staged).await;
    }
    result
}

/// A unique name beside `path`, so two installs of the same file never share one.
fn staging_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.part", uuid::Uuid::new_v4()))
}

/// The SHA-1 of everything `reader` holds, hex encoded.
pub fn sha1_of(mut reader: impl std::io::Read) -> std::io::Result<String> {
    let mut hasher = sha1::Sha1::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Whether the file at `path` has `hash`. A missing file has none.
pub async fn has_hash(path: &Path, hash: Hash<'_>) -> bool {
    use tokio::io::AsyncReadExt;
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return false;
    };
    let mut hasher = Hasher::new(hash);
    let mut buffer = vec![0; 256 * 1024];
    loop {
        match file.read(&mut buffer).await {
            Ok(0) => return hasher.matches(hash),
            Ok(read) => hasher.update(&buffer[..read]),
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_file_is_checked_against_its_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jar");
        write_whole(&path, b"hello").await.unwrap();
        // SHA-1 of "hello".
        let sha1 = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
        assert!(has_hash(&path, Hash::Sha1(sha1)).await);
        assert!(!has_hash(&path, Hash::Sha1(&"0".repeat(40))).await);
        assert!(!has_hash(&dir.path().join("missing"), Hash::Sha1(sha1)).await);
        // Nothing is left beside it.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
