use std::path::Path;

use async_zip::tokio::read::seek::ZipFileReader;
use tokio::io::BufReader;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use crate::domain::modpack::PackFormat;

#[derive(Debug, thiserror::Error)]
pub enum MrpackError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip: {0}")]
    Zip(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("download URL is not allowed: {0}")]
    DisallowedDownloadUrl(String),
}

/// Extract the `modrinth.index.json` from a `.mrpack` file.
pub async fn extract_metadata(
    mrpack_path: &Path,
) -> Result<PackFormat, MrpackError> {
    let file = tokio::fs::File::open(mrpack_path).await?;
    let mut reader = ZipFileReader::with_tokio(BufReader::new(file))
        .await
        .map_err(|e| MrpackError::Zip(e.to_string()))?;
    for i in 0..reader.file().entries().len() {
        let entry = reader.file().entries()[i]
            .filename()
            .as_str()
            .map_err(|e| MrpackError::Zip(e.to_string()))?
            .to_string();
        if entry == "modrinth.index.json" {
            let entry_reader = reader
                .reader_with_entry(i)
                .await
                .map_err(|e| MrpackError::Zip(e.to_string()))?;
            let mut buf = Vec::new();
            tokio::io::copy(&mut entry_reader.compat(), &mut buf).await?;
            return Ok(serde_json::from_slice(&buf)?);
        }
    }
    Err(MrpackError::Zip(
        "modrinth.index.json not found in mrpack".into(),
    ))
}

pub(crate) fn validate_download_url(url: &str) -> Result<(), MrpackError> {
    let parsed = url::Url::parse(url)
        .map_err(|_| MrpackError::DisallowedDownloadUrl(url.to_string()))?;
    if parsed.scheme() != "https" {
        return Err(MrpackError::DisallowedDownloadUrl(url.to_string()));
    }
    let Some(host) = parsed.host_str() else {
        return Err(MrpackError::DisallowedDownloadUrl(url.to_string()));
    };
    let allowed = host == "cdn.modrinth.com"
        || host.ends_with(".modrinth.com")
        || host == "github.com"
        || host.ends_with(".github.com")
        || host.ends_with(".githubusercontent.com");
    if !allowed {
        return Err(MrpackError::DisallowedDownloadUrl(url.to_string()));
    }
    Ok(())
}
