use std::path::{Component, Path, PathBuf};

use async_zip::tokio::read::seek::ZipFileReader;
use sha1::Digest;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::info;

use crate::domain::modpack::{EnvType, PackFile, PackFormat};

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
    #[error("sha1 mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("path traversal rejected")]
    PathTraversal,
    #[error("file has no downloads: {0}")]
    MissingDownload(String),
    #[error("download URL is not allowed: {0}")]
    DisallowedDownloadUrl(String),
    #[error("destination symlink rejected")]
    DestinationSymlink,
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

/// Install a `.mrpack` to `instance_dir/mods/`.
pub async fn install_mrpack(
    http: &reqwest::Client,
    mrpack_path: &Path,
    instance_dir: &Path,
) -> Result<PackFormat, MrpackError> {
    let metadata = extract_metadata(mrpack_path).await?;
    let mods_dir = instance_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;

    // Download server-side files
    for file in &metadata.files {
        if is_client_only(file) {
            continue;
        }
        let dest = guarded_child_path(instance_dir, &file.path)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        info!("Downloading {}", file.path);
        let download_url = file
            .downloads
            .first()
            .ok_or_else(|| MrpackError::MissingDownload(file.path.clone()))?;
        download_with_sha1(
            http,
            download_url,
            &dest,
            file.hashes.sha1.as_deref(),
        )
        .await?;
    }

    // Extract overrides/
    extract_overrides(mrpack_path, instance_dir, "overrides").await?;
    extract_overrides(mrpack_path, instance_dir, "server-overrides").await?;

    Ok(metadata)
}

fn is_client_only(file: &PackFile) -> bool {
    matches!(
        file.env.as_ref().map(|e| &e.server),
        Some(EnvType::Unsupported)
    )
}

async fn download_with_sha1(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), MrpackError> {
    validate_download_url(url)?;
    let bytes = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if let Some(expected) = expected_sha1 {
        let actual = hex::encode(sha1::Sha1::digest(&bytes));
        if actual != expected {
            return Err(MrpackError::HashMismatch {
                path: dest.display().to_string(),
                expected: expected.to_string(),
                actual,
            });
        }
    }
    ensure_no_destination_symlink(dest)?;
    let mut f = tokio::fs::File::create(dest).await?;
    f.write_all(&bytes).await?;
    Ok(())
}

fn validate_download_url(url: &str) -> Result<(), MrpackError> {
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

async fn extract_overrides(
    mrpack: &Path,
    dest: &Path,
    prefix: &str,
) -> Result<(), MrpackError> {
    let file = tokio::fs::File::open(mrpack).await?;
    let mut reader = ZipFileReader::with_tokio(BufReader::new(file))
        .await
        .map_err(|e| MrpackError::Zip(e.to_string()))?;
    for i in 0..reader.file().entries().len() {
        let entry_name = reader.file().entries()[i]
            .filename()
            .as_str()
            .map_err(|e| MrpackError::Zip(e.to_string()))?
            .to_string();
        if !entry_name.starts_with(&format!("{prefix}/")) {
            continue;
        }
        let rel = entry_name.trim_start_matches(&format!("{prefix}/"));
        if rel.is_empty() {
            continue;
        }
        let out_path = guarded_child_path(dest, rel)?;
        if let Some(parent) = out_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let entry_reader = reader
            .reader_with_entry(i)
            .await
            .map_err(|e| MrpackError::Zip(e.to_string()))?;
        ensure_no_destination_symlink(&out_path)?;
        let mut f = tokio::fs::File::create(&out_path).await?;
        tokio::io::copy(&mut entry_reader.compat(), &mut f).await?;
    }
    Ok(())
}

fn ensure_no_destination_symlink(path: &Path) -> Result<(), MrpackError> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(MrpackError::DestinationSymlink)
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(MrpackError::Io(err)),
    }
}

fn guarded_child_path(
    base: &Path,
    child: &str,
) -> Result<PathBuf, MrpackError> {
    let rel = Path::new(child);
    for component in rel.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(MrpackError::PathTraversal);
        }
    }
    Ok(base.join(rel))
}
