//! The reading side of a share: a version as a server takes it. A server's Core reads the latest
//! version of the share it follows with [`ready_version`], builds a [`ServerPack`] from it with
//! [`server_pack`], fetches its files with [`sharing_client::pool::fetch`], and installs it like
//! any pack.

use std::collections::BTreeMap;
use std::future::Future;

use bedrock::content::{
    Hashes, Index, IndexFile, Version, loader_dependency, mrpack_env,
};
use bedrock::state::ModLoader;
use contracts::backend::sharing::{
    GetLatestVersion, InstanceVersion, SharedInstanceId, VersionRead,
};
use contracts::client::{CallError, Remote, call};
use contracts::{Empty, ErrorCode, Side};

use sharing_client::install::{
    ModpackFile, Source, Step, config_update, install_plan,
};
use sharing_client::pool::{InvalidPoolEntry, PoolFile, pool_access};
use sharing_client::shares::{Unavailable, status};
use sharing_client::versions::modpack_id;

/// A share version as a server takes it. `S` is how the caller gets a modpack file's bytes.
#[derive(Debug, Clone)]
pub struct ServerPack<S> {
    pub version: i32,
    /// `modrinth.index.json` for the Modrinth mods the version adds over its modpack. Core
    /// writes it with the modpack's own files, and its pack install checks it.
    pub index: Index,
    /// The modpack's files the server gets, Modrinth's or not: index entries, overrides and
    /// config files alike, at the paths the modpack puts them.
    pub modpack_files: Vec<ModpackFile<S>>,
    /// The version's files that live in the pool, by their path in the server folder.
    pub files: Vec<(String, PoolFile)>,
}

/// Modrinth's API as the caller reaches it, so each side uses its own client and cache.
pub trait Modrinth {
    type Error;
    /// How the caller gets a modpack file's bytes.
    type Source;

    /// The versions among `ids` that Modrinth has.
    fn versions(
        &self,
        ids: &[String],
    ) -> impl Future<Output = Result<Vec<Version>, Self::Error>> + Send;

    /// Every file the modpack version `id` places, by path, with the Modrinth version of each
    /// one Modrinth knows. The server's pack install decides which of them it runs.
    fn modpack_files(
        &self,
        id: &str,
    ) -> impl Future<
        Output = Result<Vec<ModpackFile<Self::Source>>, Self::Error>,
    > + Send;
}

/// Why a share's latest version cannot go to a server now.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Backend(#[from] CallError),
    /// The backend refused the read (401 or 403), with the code it named (`Unknown` when it
    /// sent no error body) and its description.
    #[error("{description}")]
    Denied {
        code: ErrorCode,
        description: String,
    },
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
    version: InstanceVersion,
}

/// The latest version of the share `share_id`, when it is ready for a server, read as the user
/// whose `read_ticket` this is (as the Core's owner without one). A Core reads this before it
/// accepts an install, so a version that cannot go fails the request itself.
pub async fn ready_version(
    backend: &Remote<'_>,
    share_id: &str,
    read_ticket: Option<String>,
) -> Result<ReadyVersion, ReadError> {
    let id = SharedInstanceId {
        id: share_id.to_owned(),
    };
    let query = VersionRead { read_ticket };
    let version = match call::<GetLatestVersion>(backend, &id, &query, Empty)
        .await
    {
        Ok(version) => version,
        Err(error) if Unavailable::of(&error) == Some(Unavailable::Deleted) => {
            return Err(ReadError::NoVersion);
        }
        Err(error) if matches!(status(&error), Some(401 | 403)) => {
            let (code, description) = match &error {
                CallError::Api { error, .. } => {
                    (error.error, error.description.clone())
                }
                _ => (ErrorCode::Unknown, String::new()),
            };
            let description = match description.is_empty() {
                true => format!(
                    "The backend refused the read with status {}.",
                    status(&error).unwrap_or_default()
                ),
                false => description,
            };
            return Err(ReadError::Denied { code, description });
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

/// `version` as a server takes it, by [`install_plan`] for [`Side::Server`]: the Modrinth mods
/// it adds, in a pack index named `name`, the files of its modpack (less the ones the owner
/// removed or the share carries itself), and the mods and config files in the pool. Client-only
/// Modrinth mods are listed but marked unsupported on servers. The pack is the server's whole
/// record of the share, so every config is in it; the server's pack install compares it with
/// what is on disk and keeps the owner's edits.
pub async fn server_pack<M: Modrinth>(
    ReadyVersion { share_id, version }: ReadyVersion,
    name: &str,
    modrinth: &M,
) -> Result<ServerPack<M::Source>, ServerPackError<M::Error>> {
    let content = &version.manifest.content;
    let modpack = match modpack_id(&version) {
        Some(modpack) => modrinth
            .modpack_files(modpack)
            .await
            .map_err(ServerPackError::Modrinth)?,
        None => Vec::new(),
    };
    let plan = install_plan(
        &version,
        modpack,
        config_update(&version.manifest.external_files, None),
        Side::Server,
    );
    let ids = &plan.modrinth_ids;

    let versions = if ids.is_empty() {
        Vec::new()
    } else {
        modrinth
            .versions(ids)
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

    let access = pool_access(&version, &share_id);
    let mut pool_files = Vec::new();
    for (path, file) in plan.pool_files() {
        let access = access.clone().ok_or(ServerPackError::PoolUnavailable)?;
        pool_files.push((path.to_owned(), PoolFile::new(file, access)?));
    }
    let modpack_files = plan
        .steps
        .into_iter()
        .filter_map(|step| match step {
            Step::Place {
                source: Source::Modpack(file),
                ..
            } => Some(file),
            _ => None,
        })
        .collect();

    Ok(ServerPack {
        version: version.version,
        modpack_files,
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
