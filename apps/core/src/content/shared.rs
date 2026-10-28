//! A share version as a pack. A server follows an App instance by applying versions of the
//! instance's share, like one more member. The App sends the version: a Modrinth pack index for
//! the files on Modrinth, and the pool entries of the rest. Core fetches those through the store
//! API of the version's holders, as a member's App does, decrypts and checks them, and writes an
//! `.mrpack` with them as overrides, which installs like any other pack.
//!
//! Core only ever asks a holder for a file through the store API, so where and how the pool
//! keeps files does not matter here.

use std::path::{Path, PathBuf};

use amberite_pool::Decryptor;
use futures::TryStreamExt;
use serde::Deserialize;
use sha1::Digest;
use tokio::io::AsyncWriteExt;
use tokio_util::io::StreamReader;
use tracing::warn;

use super::{Content, Error, install::MAX_PACK};
use crate::installs::shared::SharedDirs;

/// A share version as the App sends it.
#[derive(Debug, Clone, Deserialize)]
pub struct SharedVersion {
    pub shared_instance_id: String,
    pub version: i64,
    /// `modrinth.index.json`, written as it is. The install checks it.
    pub index: serde_json::Value,
    /// Where the files below are, absent when the version has none.
    #[serde(default)]
    pub pool: Option<PoolAccess>,
    #[serde(default)]
    pub files: Vec<PoolFile>,
}

/// A version's holders and the fetch ticket for it, as the backend gave them to the App.
#[derive(Debug, Clone, Deserialize)]
pub struct PoolAccess {
    /// R, in hex.
    pub root: String,
    pub holders: Vec<Holder>,
    pub ticket: String,
}

/// A Core holding a version. `address` is tried first, then `public_address`.
#[derive(Debug, Clone, Deserialize)]
pub struct Holder {
    pub address: String,
    #[serde(default)]
    pub public_address: Option<String>,
    pub cert_fingerprint: String,
}

/// One file of the version that lives in the pool, by its path in the server folder.
#[derive(Debug, Clone, Deserialize)]
pub struct PoolFile {
    pub path: String,
    /// C, in hex.
    pub object: String,
    pub object_size: u64,
    /// P, in hex.
    pub sha256: String,
    /// The file's own key, in hex; absent for content files, whose key derives from P.
    #[serde(default)]
    pub key: Option<String>,
}

fn hash(text: &str) -> Result<[u8; 32], Error> {
    let mut hash = [0; 32];
    hex::decode_to_slice(text, &mut hash)
        .map_err(|_| Error::Invalid(format!("{text} is not a hash.")))?;
    Ok(hash)
}

/// Fetch the version's files and write it as an `.mrpack` in the shared store. Returns the
/// pack's SHA-1.
pub async fn write_pack(
    content: &Content,
    server_id: &str,
    version: &SharedVersion,
) -> Result<String, Error> {
    let total: u64 = version.files.iter().map(|file| file.object_size).sum();
    if total > MAX_PACK {
        return Err(Error::Invalid(
            "The instance's files are too large for one server.".into(),
        ));
    }
    let staging = content
        .paths
        .staging(server_id)
        .join(uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&staging).await?;
    let result = async {
        let mut fetched = Vec::with_capacity(version.files.len());
        if !version.files.is_empty() {
            let pool = version.pool.as_ref().ok_or_else(|| {
                Error::Invalid(
                    "The version has files but nowhere to fetch them.".into(),
                )
            })?;
            for (n, file) in version.files.iter().enumerate() {
                let path = staging.join(n.to_string());
                fetch(pool, file, &path).await?;
                fetched.push((file.path.clone(), path));
            }
        }
        let dirs = SharedDirs::new(content.paths.root());
        let packed = staging.join("pack.mrpack");
        let index = serde_json::to_vec(&version.index)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let sha1 = {
            let packed = packed.clone();
            tokio::task::spawn_blocking(move || zip(&packed, &index, &fetched))
                .await
                .map_err(std::io::Error::other)??
        };
        let stored = dirs.pack(&sha1);
        if let Some(parent) = stored.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::rename(&packed, &stored).await?;
        Ok(sha1)
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result
}

/// Fetch one file from the first holder that serves it correctly, decrypted and checked against
/// both C and P, into `path`.
async fn fetch(
    pool: &PoolAccess,
    file: &PoolFile,
    path: &Path,
) -> Result<(), Error> {
    let root = hash(&pool.root)?;
    let object = hash(&file.object)?;
    let plain = hash(&file.sha256)?;
    let key = match &file.key {
        Some(key) => hash(key)?,
        None => amberite_pool::content_key(&plain),
    };
    let mut last = format!("{} is on no holder of the version.", file.path);
    for holder in &pool.holders {
        let client = match crate::store::pull::pinned(&holder.cert_fingerprint)
        {
            Ok(client) => client,
            Err(error) => {
                last = error.description;
                continue;
            }
        };
        for address in std::iter::once(&holder.address)
            .chain(holder.public_address.as_ref())
        {
            // An `https://ip:port`, so the ticket only goes over TLS pinned to the holder. A
            // private address is allowed: a holder on the Core's own network is reached there.
            let base = match crate::store::pull::address(address, true) {
                Ok(base) => base,
                Err(error) => {
                    last = error.description;
                    continue;
                }
            };
            let url = format!(
                "{}/store/versions/{}/objects/{}",
                base.as_str().trim_end_matches('/'),
                hex::encode(root),
                hex::encode(object)
            );
            match fetch_from(
                &client,
                &url,
                &pool.ticket,
                file,
                &key,
                object,
                plain,
                path,
            )
            .await
            {
                Ok(()) => return Ok(()),
                Err(error) => {
                    warn!(%url, %error, "could not fetch a shared file");
                    last = error;
                }
            }
        }
    }
    let _ = tokio::fs::remove_file(path).await;
    Err(Error::Invalid(format!(
        "Could not fetch {}: {last}",
        file.path
    )))
}

#[allow(clippy::too_many_arguments)]
async fn fetch_from(
    client: &reqwest::Client,
    url: &str,
    ticket: &str,
    file: &PoolFile,
    key: &[u8; 32],
    object: [u8; 32],
    plain: [u8; 32],
    path: &Path,
) -> Result<(), String> {
    let response = client
        .get(url)
        .bearer_auth(ticket)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    if response
        .content_length()
        .is_some_and(|length| length != file.object_size)
    {
        return Err("the holder sent another size".into());
    }
    let body = StreamReader::new(
        response.bytes_stream().map_err(std::io::Error::other),
    );
    let mut out = tokio::fs::File::create(path)
        .await
        .map_err(|error| error.to_string())?;
    Decryptor::new(key, object, plain)
        .run(body, &mut out)
        .await
        .map_err(|error| error.to_string())?;
    out.flush().await.map_err(|error| error.to_string())
}

/// Write `index` and the fetched files, under `overrides/`, as an `.mrpack` at `path`. Returns
/// its SHA-1.
fn zip(
    path: &PathBuf,
    index: &[u8],
    files: &[(String, PathBuf)],
) -> Result<String, Error> {
    use std::io::Write;

    let zip_error =
        |error: zip::result::ZipError| Error::Io(std::io::Error::other(error));
    // The files are mostly jars, already compressed.
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path)?);
    zip.start_file("modrinth.index.json", options)
        .map_err(zip_error)?;
    zip.write_all(index)?;
    for (name, fetched) in files {
        zip.start_file(format!("overrides/{name}"), options)
            .map_err(zip_error)?;
        std::io::copy(&mut std::fs::File::open(fetched)?, &mut zip)?;
    }
    zip.finish().map_err(zip_error)?;
    let mut hasher = sha1::Sha1::default();
    std::io::copy(&mut std::fs::File::open(path)?, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}
