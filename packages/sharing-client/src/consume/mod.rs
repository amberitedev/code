//! The reading side of a share: a version as a server takes it. A server's Core reads the latest
//! version of the share it follows with [`ready_version`], builds a [`ServerPack`] from it with
//! [`server_pack`], fetches its files with [`crate::pool::fetch`], and installs it like any
//! pack.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;

use bedrock::content::{
    Hashes, Index, IndexFile, Version, loader_dependency, mrpack_env,
};
use bedrock::state::ModLoader;
use reqwest::StatusCode;

use crate::backend::{Backend, BackendError, Unavailable};
use crate::pool::{InvalidPoolEntry, PoolFile};
use crate::rules::CONFIG_FILE_TYPE;
use crate::versions::ShareVersion;

/// A share version as a server takes it.
#[derive(Debug, Clone)]
pub struct ServerPack {
    pub shared_instance_id: String,
    pub version: i32,
    /// `modrinth.index.json` for the files on Modrinth. Core writes it as it is, and its pack
    /// install checks it.
    pub index: Index,
    /// The version's files that live in the pool, by their path in the server folder.
    pub files: Vec<(String, PoolFile)>,
}

/// Modrinth's API as the caller reaches it, so each side uses its own client and cache.
pub trait Modrinth {
    type Error;

    /// The versions among `ids` that Modrinth has.
    fn versions(
        &self,
        ids: &[String],
    ) -> impl Future<Output = Result<Vec<Version>, Self::Error>> + Send;

    /// Every file the modpack version `id` puts on a server, by path, with the Modrinth version
    /// of each one Modrinth knows.
    fn modpack_files(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Vec<ModpackFile>, Self::Error>> + Send;
}

#[derive(Debug, Clone)]
pub struct ModpackFile {
    pub path: String,
    pub version_id: Option<String>,
}

/// Why a share's latest version cannot go to a server now.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Backend(#[from] BackendError),
    /// The backend refused the read (401 or 403), with the `code` it named, or its `error` kind
    /// when it named none, and its description.
    #[error("{description}")]
    Denied { code: String, description: String },
    #[error("The instance's share has no version.")]
    NoVersion,
    #[error(
        "The instance's latest version is still uploading. Try again in a moment."
    )]
    NotReady,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerPackError<E> {
    #[error("Modrinth version {0} of the share was not found.")]
    MissingVersion(String),
    #[error(transparent)]
    InvalidPoolEntry(#[from] InvalidPoolEntry),
    #[error("The share's files are not in the pool.")]
    PoolUnavailable,
    #[error("Modrinth: {0}")]
    Modrinth(E),
}

/// The latest version of a share, read with [`ready_version`] and ready for [`server_pack`].
pub struct ReadyVersion {
    share_id: String,
    version: ShareVersion,
}

/// The latest version of the share `share_id`, when it is ready for a server. A Core reads this
/// before it accepts an install, so a version that cannot go fails the request itself.
pub async fn ready_version(
    backend: &Backend,
    share_id: &str,
) -> Result<ReadyVersion, ReadError> {
    let version = match backend.version(share_id, None).await {
        Ok(version) => version,
        Err(BackendError::Unavailable {
            reason: Unavailable::Deleted,
            ..
        }) => return Err(ReadError::NoVersion),
        Err(error)
            if matches!(
                error.status(),
                Some(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
            ) =>
        {
            let status = error.status().unwrap_or_default();
            let body = error.body().cloned().unwrap_or_default();
            let description = if body.description.is_empty() {
                format!("The backend refused the read with status {status}.")
            } else {
                body.description.clone()
            };
            return Err(ReadError::Denied {
                code: body.code().to_owned(),
                description,
            });
        }
        Err(error) => return Err(error.into()),
    };
    if !version.ready {
        return Err(ReadError::NotReady);
    }
    Ok(ReadyVersion {
        share_id: share_id.to_string(),
        version,
    })
}

/// `version` as a server takes it: the Modrinth mods, in a pack index named `name`, and the mods
/// and config files in the pool. A version based on a modpack takes the modpack's files too,
/// less the ones the owner removed or the share carries itself. Client-only Modrinth mods are
/// listed but marked unsupported on servers.
pub async fn server_pack<M: Modrinth>(
    ReadyVersion { share_id, version }: ReadyVersion,
    name: &str,
    modrinth: &M,
) -> Result<ServerPack, ServerPackError<M::Error>> {
    let content = &version.content;
    let modpack = version.modpack_id();
    let mut ids = content
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
                        && !content.removed_files.iter().any(|removed| {
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
        // A version without a file to install from is left out.
        .filter_map(|version| {
            let file = version.primary()?;
            Some(IndexFile {
                path: format!("mods/{}", file.filename),
                hashes: Hashes {
                    sha1: file.hashes.sha1.clone(),
                    sha512: file.hashes.sha512.clone(),
                },
                env: Some(mrpack_env(version.environment)),
                downloads: vec![file.url.clone()],
                file_size: file.size,
            })
        })
        .collect();

    let mut dependencies = BTreeMap::from([(
        "minecraft".to_owned(),
        content.game_version.clone(),
    )]);
    if let Some(loader) =
        loader_dependency(ModLoader::from_string(&content.loader))
            .filter(|_| !content.loader_version.is_empty())
    {
        dependencies.insert(loader.to_owned(), content.loader_version.clone());
    }

    let access = version.pool_access(&share_id);
    let mut pool_files = Vec::new();
    for file in &version.external_files {
        let Some(path) = server_path(&file.file_type, &file.file_name) else {
            continue;
        };
        let access = access.clone().ok_or(ServerPackError::PoolUnavailable)?;
        pool_files.push((path, PoolFile::new(file, access)?));
    }

    Ok(ServerPack {
        shared_instance_id: share_id,
        version: version.version,
        index: Index {
            format_version: Index::FORMAT_VERSION,
            game: Index::GAME.to_owned(),
            version_id: version.version.to_string(),
            name: name.to_string(),
            summary: None,
            files,
            dependencies,
        },
        files: pool_files,
    })
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
