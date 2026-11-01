//! The reading side of a share: a version as a server takes it, and fetching a version's files
//! from the pool. The App builds a [`ServerPack`] from the share's latest version and hands it to
//! the server's Core, which fetches its files with [`fetch`] and installs it like any pack.

use std::future::Future;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::crypto::{self, parse_hash};
use crate::push::CONFIG_FILE_TYPE;
use crate::store::{FetchError, PoolCore, PoolObject, StoreClient};

/// A share version as a server takes it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ServerPack {
    pub shared_instance_id: String,
    pub version: i32,
    /// `modrinth.index.json` for the files on Modrinth. Core writes it as it is, and its pack
    /// install checks it.
    #[ts(type = "unknown")]
    pub index: serde_json::Value,
    /// Where `files` are, absent when there are none.
    #[serde(default)]
    pub pool: Option<PoolAccess>,
    #[serde(default)]
    pub files: Vec<PoolFile>,
}

/// A version's holders and the fetch ticket for it, as the backend gives them.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PoolAccess {
    /// R, in hex.
    pub root: String,
    pub holders: Vec<PoolCore>,
    pub ticket: String,
}

/// A file of the version that lives in the pool, by its path in the server folder.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PoolFile {
    pub path: String,
    /// C, in hex.
    pub object: String,
    pub object_size: u64,
    /// P, in hex.
    pub sha256: String,
    /// The file's own key, in hex. Absent for content files, whose key derives from P.
    #[serde(default)]
    pub key: Option<String>,
}

/// Where a manifest entry goes on a server, or `None` when servers do not take it. A server gets
/// mods and the config files a version carries; resource packs, shaders, and the rest are for
/// players.
pub fn server_path(file_type: &str, file_name: &str) -> Option<String> {
    match file_type {
        CONFIG_FILE_TYPE => Some(file_name.to_string()),
        "mod" => Some(format!("mods/{file_name}")),
        _ => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConsumeError {
    #[error("{0} is not a hash")]
    InvalidHash(String),
    #[error(transparent)]
    Fetch(#[from] FetchError<std::io::Error>),
}

/// Fetches one file from the first holder that serves it correctly, decrypted and checked
/// against both C and P, and hands its plain bytes to `sink`. `sink` is called afresh for each
/// holder. Holders that served bad bytes are logged by the store client.
pub async fn fetch<Fut>(
    store: &StoreClient,
    access: &PoolAccess,
    file: &PoolFile,
    sink: impl FnMut(tokio::io::DuplexStream) -> Fut,
) -> Result<(), ConsumeError>
where
    Fut: Future<Output = Result<(), std::io::Error>>,
{
    let hash = |text: &str| {
        parse_hash(text)
            .ok_or_else(|| ConsumeError::InvalidHash(text.to_string()))
    };
    let root = hex::encode(hash(&access.root)?);
    let plain = hash(&file.sha256)?;
    let object = PoolObject {
        root: &root,
        holders: &access.holders,
        ticket: &access.ticket,
        object: hash(&file.object)?,
        object_size: file.object_size,
        plain,
        key: match &file.key {
            Some(key) => hash(key)?,
            None => crypto::content_key(&plain),
        },
    };
    let mut faults = Vec::new();
    store
        .fetch_file(&object, &|_| {}, &mut faults, sink)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_take_mods_and_configs_only() {
        assert_eq!(server_path("mod", "a.jar").as_deref(), Some("mods/a.jar"));
        assert_eq!(
            server_path("config", "config/a.json").as_deref(),
            Some("config/a.json")
        );
        assert_eq!(server_path("resourcepack", "a.zip"), None);
        assert_eq!(server_path("shader", "a.zip"), None);
    }
}
