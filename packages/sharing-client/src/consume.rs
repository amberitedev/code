//! The reading side of a share: a version as a server takes it, and fetching a version's files
//! from the pool. A server's Core builds a [`ServerPack`] from the latest version of the share
//! it follows with [`server_pack`], fetches its files with [`fetch`], and installs it like any
//! pack.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::crypto::{self, parse_hash};
use crate::push::{Backend, CONFIG_FILE_TYPE, PushError};
use crate::store::{FetchError, PoolCore, PoolObject, StoreClient};

/// A share version as a server takes it.
#[derive(Debug, Clone)]
pub struct ServerPack {
    pub shared_instance_id: String,
    pub version: i32,
    /// `modrinth.index.json` for the files on Modrinth. Core writes it as it is, and its pack
    /// install checks it.
    pub index: PackIndex,
    /// Where `files` are, absent when there are none.
    pub pool: Option<PoolAccess>,
    pub files: Vec<PoolFile>,
}

/// A version's holders and the fetch ticket for it, as the backend gives them.
#[derive(Debug, Clone)]
pub struct PoolAccess {
    /// R, in hex.
    pub root: String,
    pub holders: Vec<PoolCore>,
    pub ticket: String,
}

/// A file of the version that lives in the pool, by its path in the server folder.
#[derive(Debug, Clone)]
pub struct PoolFile {
    pub path: String,
    /// C, in hex.
    pub object: String,
    pub object_size: u64,
    /// P, in hex.
    pub sha256: String,
    /// The file's own key, in hex. Absent for content files, whose key derives from P.
    pub key: Option<String>,
}

/// `modrinth.index.json` of a [`ServerPack`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIndex {
    format_version: u32,
    game: &'static str,
    version_id: String,
    name: String,
    files: Vec<PackIndexFile>,
    dependencies: BTreeMap<&'static str, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackIndexFile {
    path: String,
    hashes: BTreeMap<&'static str, String>,
    env: BTreeMap<&'static str, &'static str>,
    downloads: Vec<String>,
    file_size: u64,
}

/// Modrinth's API as the caller reaches it, so each side uses its own client and cache.
pub trait Modrinth {
    type Error;

    /// The versions among `ids` that Modrinth has.
    fn versions(
        &self,
        ids: &[String],
    ) -> impl Future<Output = Result<Vec<ModrinthVersion>, Self::Error>> + Send;

    /// Every file the modpack version `id` puts on a server, by path, with the Modrinth version
    /// of each one Modrinth knows.
    fn modpack_files(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Vec<ModpackFile>, Self::Error>> + Send;
}

/// A Modrinth version, as far as a server pack needs it.
#[derive(Debug, Clone)]
pub struct ModrinthVersion {
    pub id: String,
    pub loaders: Vec<String>,
    /// v3's `environment`, such as `client_only`.
    pub environment: Option<String>,
    /// The file it is installed from. A version without one is left out.
    pub file: Option<ModrinthFile>,
}

#[derive(Debug, Clone)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    pub sha1: String,
    pub sha512: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct ModpackFile {
    pub path: String,
    pub version_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerPackError<E> {
    #[error(transparent)]
    Backend(#[from] PushError),
    #[error("The instance's share has no version.")]
    NoVersion,
    #[error(
        "The instance's latest version is still uploading. Try again in a moment."
    )]
    NotReady,
    #[error("Modrinth version {0} of the share was not found.")]
    MissingVersion(String),
    #[error("The share's file {0} has an invalid pool entry.")]
    InvalidPoolEntry(String),
    #[error("The share's files are not in the pool.")]
    PoolUnavailable,
    #[error("Modrinth: {0}")]
    Modrinth(E),
}

/// A share version as the backend gives it to a reader.
#[derive(Deserialize)]
struct SharedVersion {
    version: i32,
    ready: bool,
    #[serde(default)]
    modrinth_ids: Vec<String>,
    #[serde(default)]
    external_files: Vec<SharedFile>,
    #[serde(default)]
    modpack_id: Option<String>,
    #[serde(default)]
    removed_files: Vec<RemovedFile>,
    game_version: String,
    loader: String,
    #[serde(default)]
    loader_version: String,
    /// R, in hex.
    #[serde(default)]
    root: Option<String>,
    /// Where readers fetch the files, once the version is ready.
    #[serde(default)]
    pool: Option<SharedPool>,
}

#[derive(Deserialize)]
struct SharedFile {
    file_name: String,
    file_type: String,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    object: Option<String>,
    #[serde(default)]
    object_size: Option<u64>,
    #[serde(default)]
    key: Option<String>,
}

#[derive(Deserialize)]
struct SharedPool {
    holders: Vec<PoolCore>,
    ticket: String,
}

/// A file of the version's modpack that the owner removed from the instance.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RemovedFile {
    Version { version_id: String },
    Path { parent: String, filename: String },
}

impl RemovedFile {
    fn matches(&self, version_id: Option<&str>, path: &str) -> bool {
        match self {
            Self::Version {
                version_id: removed,
            } => version_id == Some(removed.as_str()),
            Self::Path { parent, filename } => {
                let enabled = |name: &str| {
                    name.strip_suffix(".disabled").unwrap_or(name).to_string()
                };
                enabled(path) == format!("{parent}/{}", enabled(filename))
            }
        }
    }
}

/// The latest version of the share `share_id`, as a server takes it: the Modrinth mods, in a
/// pack index named `name`, and the mods and config files in the pool. A version based on a
/// modpack takes the modpack's files too, less the ones the owner removed or the share carries
/// itself. Client-only Modrinth mods are listed but marked unsupported on servers.
pub async fn server_pack<M: Modrinth>(
    backend: &Backend,
    share_id: &str,
    name: &str,
    modrinth: &M,
) -> Result<ServerPack, ServerPackError<M::Error>> {
    let version: SharedVersion = backend
        .version(share_id, None)
        .await?
        .ok_or(ServerPackError::NoVersion)?;
    if !version.ready {
        return Err(ServerPackError::NotReady);
    }

    let modpack = version.modpack_id.as_deref().filter(|id| !id.is_empty());
    let mut ids = version
        .modrinth_ids
        .iter()
        .filter(|id| modpack != Some(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if let Some(modpack) = modpack {
        let carried = version
            .external_files
            .iter()
            .filter_map(|file| server_path(&file.file_type, &file.file_name))
            .collect::<HashSet<_>>();
        let files = modrinth
            .modpack_files(modpack)
            .await
            .map_err(ServerPackError::Modrinth)?;
        ids.extend(
            files
                .into_iter()
                .filter(|file| {
                    !carried.contains(&file.path)
                        && !version.removed_files.iter().any(|removed| {
                            removed
                                .matches(file.version_id.as_deref(), &file.path)
                        })
                })
                .filter_map(|file| file.version_id),
        );
    }
    ids.sort_unstable();
    ids.dedup();

    let versions = if ids.is_empty() {
        Vec::new()
    } else {
        modrinth
            .versions(&ids)
            .await
            .map_err(ServerPackError::Modrinth)?
    };
    if let Some(missing) = ids
        .iter()
        .find(|id| !versions.iter().any(|version| &version.id == *id))
    {
        return Err(ServerPackError::MissingVersion(missing.clone()));
    }
    let files = versions
        .into_iter()
        .filter(|version| {
            version.loaders.iter().any(|loader| {
                matches!(
                    loader.as_str(),
                    "fabric" | "forge" | "quilt" | "neoforge"
                )
            })
        })
        .filter_map(|version| {
            let file = version.file?;
            let mut hashes = BTreeMap::from([("sha1", file.sha1)]);
            hashes.extend(file.sha512.map(|sha512| ("sha512", sha512)));
            Some(PackIndexFile {
                path: format!("mods/{}", file.filename),
                hashes,
                env: BTreeMap::from([
                    ("client", "required"),
                    ("server", server_side(version.environment.as_deref())),
                ]),
                downloads: vec![file.url],
                file_size: file.size,
            })
        })
        .collect();

    let mut dependencies =
        BTreeMap::from([("minecraft", version.game_version.clone())]);
    if let Some(loader) = match version.loader.as_str() {
        "fabric" => Some("fabric-loader"),
        "quilt" => Some("quilt-loader"),
        "forge" => Some("forge"),
        "neoforge" => Some("neoforge"),
        _ => None,
    }
    .filter(|_| !version.loader_version.is_empty())
    {
        dependencies.insert(loader, version.loader_version.clone());
    }

    let mut pool_files = Vec::new();
    for file in version.external_files {
        let Some(path) = server_path(&file.file_type, &file.file_name) else {
            continue;
        };
        let invalid =
            || ServerPackError::InvalidPoolEntry(file.file_name.clone());
        pool_files.push(PoolFile {
            path,
            object: file.object.clone().ok_or_else(invalid)?,
            object_size: file.object_size.ok_or_else(invalid)?,
            sha256: file.sha256.clone().ok_or_else(invalid)?,
            key: file.key.clone(),
        });
    }
    let pool = match (version.pool, version.root) {
        _ if pool_files.is_empty() => None,
        (Some(pool), Some(root)) => Some(PoolAccess {
            root,
            holders: pool.holders,
            ticket: pool.ticket,
        }),
        _ => return Err(ServerPackError::PoolUnavailable),
    };

    Ok(ServerPack {
        shared_instance_id: share_id.to_string(),
        version: version.version,
        index: PackIndex {
            format_version: 1,
            game: "minecraft",
            version_id: version.version.to_string(),
            name: name.to_string(),
            files,
            dependencies,
        },
        pool,
        files: pool_files,
    })
}

/// Whether a server runs a Modrinth version, as a pack index says it.
fn server_side(environment: Option<&str>) -> &'static str {
    match environment {
        Some("client_only" | "singleplayer_only") => "unsupported",
        _ => "required",
    }
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

    #[test]
    fn a_removed_modpack_file_matches_enabled_or_not() {
        let removed = RemovedFile::Path {
            parent: "mods".into(),
            filename: "a.jar.disabled".into(),
        };
        assert!(removed.matches(None, "mods/a.jar"));
        assert!(!removed.matches(None, "mods/b.jar"));
        let removed = RemovedFile::Version {
            version_id: "v".into(),
        };
        assert!(removed.matches(Some("v"), "mods/a.jar"));
        assert!(!removed.matches(None, "mods/a.jar"));
    }
}
