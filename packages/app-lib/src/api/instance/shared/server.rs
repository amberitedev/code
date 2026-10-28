//! Servers follow an instance through its share, as one more member. The App hands a server's
//! Core the share's latest version as the backend gave it: a Modrinth pack index for the files
//! on Modrinth, and the pool entries of the rest. Core fetches those itself, through the store
//! API, and installs them (`content::shared` in Core). No file goes from the App to the server.

use super::client::*;
use super::publish::{
    owned_shared_instance, remote_publish_content, shared_attachment,
};
use super::*;
use crate::self_hosted::pool::store::PoolCore;
use crate::state::VersionEnvironment;

/// A share version as a server's Core takes it.
#[derive(Debug, Serialize)]
pub struct SharedInstanceServerPack {
    pub shared_instance_id: String,
    pub version: i32,
    /// `modrinth.index.json` for the files on Modrinth.
    pub index: ServerPackIndex,
    /// Where `files` are, absent when there are none.
    pub pool: Option<ServerPackPool>,
    pub files: Vec<ServerPackFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackIndex {
    format_version: u32,
    game: &'static str,
    version_id: String,
    name: String,
    files: Vec<ServerPackIndexFile>,
    dependencies: HashMap<&'static str, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerPackIndexFile {
    path: String,
    hashes: HashMap<String, String>,
    env: HashMap<&'static str, &'static str>,
    downloads: Vec<String>,
    file_size: u32,
}

#[derive(Debug, Serialize)]
pub struct ServerPackPool {
    pub root: String,
    pub holders: Vec<PoolCore>,
    pub ticket: String,
}

/// A file of the version that lives in the pool, by its path in the server folder.
#[derive(Debug, Serialize)]
pub struct ServerPackFile {
    pub path: String,
    pub object: String,
    pub object_size: u64,
    pub sha256: String,
    /// Absent for content files, whose key derives from `sha256`.
    pub key: Option<String>,
}

/// The instance's share, created with its first version when it has none, so the instance can
/// run servers. A share nobody else is in is still a share; it just has no members.
#[tracing::instrument]
pub async fn ensure_shared_instance(
    instance_id: &str,
) -> crate::Result<SharedInstanceAttachment> {
    let state = State::get().await?;
    if let Some(attachment) = shared_attachment(instance_id, &state).await? {
        return Ok(attachment);
    }
    let (_, attachment) = owned_shared_instance(instance_id, &state).await?;
    emit_instance(instance_id, InstancePayloadType::Edited).await?;
    Ok(attachment)
}

/// The share's latest version, as a server takes it. Only mods and the shared config files go
/// to a server: resource packs and shaders are for players.
#[tracing::instrument]
pub async fn get_shared_instance_server_pack(
    instance_id: &str,
) -> crate::Result<SharedInstanceServerPack> {
    let state = State::get().await?;
    let attachment = shared_attachment(instance_id, &state).await?.ok_or(
        crate::ErrorKind::InputError("The instance is not shared.".into()),
    )?;
    let version = get_latest_remote_version(&attachment.id, &state).await?;
    if !version.ready {
        bail!(crate::ErrorKind::InputError(
            "The instance's latest version is still uploading. Try again in a moment."
                .into(),
        ));
    }

    let (version_ids, _) =
        remote_publish_content(&version, true, &state).await?;
    let ids = version_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let (versions, environments) = tokio::try_join!(
        CachedEntry::get_version_many(
            &ids,
            None,
            &state.pool,
            &state.api_semaphore
        ),
        CachedEntry::get_version_v3_many(
            &ids,
            None,
            &state.pool,
            &state.api_semaphore
        ),
    )?;
    if let Some(missing) = ids
        .iter()
        .find(|id| !versions.iter().any(|version| version.id == **id))
    {
        bail!(crate::ErrorKind::InputError(format!(
            "shared instance version {missing} was not found"
        )));
    }
    let environments = environments
        .into_iter()
        .map(|version| (version.id, version.environment))
        .collect::<HashMap<_, _>>();
    let files = versions
        .into_iter()
        .filter(|version| {
            ProjectType::get_from_loaders(version.loaders.clone())
                == Some(ProjectType::Mod)
        })
        .filter_map(|version| {
            let environment = environments.get(&version.id).copied().flatten();
            let file = version
                .files
                .iter()
                .find(|file| file.primary)
                .or(version.files.first())?
                .clone();
            Some(ServerPackIndexFile {
                path: format!("mods/{}", file.filename),
                hashes: file.hashes,
                env: HashMap::from([
                    ("client", "required"),
                    ("server", server_side(environment)),
                ]),
                downloads: vec![file.url],
                file_size: file.size,
            })
        })
        .collect();

    let mut dependencies =
        HashMap::from([("minecraft", version.game_version.clone())]);
    if let Some(loader) = match version.loader {
        ModLoader::Fabric => Some("fabric-loader"),
        ModLoader::Quilt => Some("quilt-loader"),
        ModLoader::Forge => Some("forge"),
        ModLoader::NeoForge => Some("neoforge"),
        ModLoader::Vanilla => None,
    } {
        dependencies.insert(loader, version.loader_version.clone());
    }

    let mut pool_files = Vec::new();
    for file in &version.external_files {
        let path = match file.file_type.as_str() {
            CONFIG_FILE_TYPE => file.file_name.clone(),
            "mod" => format!("mods/{}", file.file_name),
            _ => continue,
        };
        let invalid = || {
            crate::ErrorKind::InputError(format!(
                "shared instance file {} has an invalid pool entry",
                file.file_name
            ))
        };
        pool_files.push(ServerPackFile {
            path,
            object: file.object.clone().ok_or_else(invalid)?,
            object_size: file.object_size.ok_or_else(invalid)?,
            sha256: file.sha256.clone().ok_or_else(invalid)?,
            key: file.key.clone(),
        });
    }
    let pool = match (version.pool, version.root) {
        _ if pool_files.is_empty() => None,
        (Some(pool), Some(root)) => Some(ServerPackPool {
            root,
            holders: pool.holders,
            ticket: pool.ticket,
        }),
        _ => bail!(crate::ErrorKind::PoolFilesUnavailable),
    };

    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    Ok(SharedInstanceServerPack {
        shared_instance_id: attachment.id,
        version: version.version,
        index: ServerPackIndex {
            format_version: 1,
            game: "minecraft",
            version_id: version.version.to_string(),
            name: metadata.instance.name,
            files,
            dependencies,
        },
        pool,
        files: pool_files,
    })
}

/// Whether a server runs a Modrinth version, as a pack index says it.
fn server_side(environment: Option<VersionEnvironment>) -> &'static str {
    match environment {
        Some(
            VersionEnvironment::ClientOnly
            | VersionEnvironment::SingleplayerOnly,
        ) => "unsupported",
        _ => "required",
    }
}

/// The Modrinth versions among `ids` that players never run, so a change to one is for servers
/// only.
pub(super) async fn server_only_versions(
    ids: impl IntoIterator<Item = &String>,
    state: &State,
) -> crate::Result<HashSet<String>> {
    let mut ids = ids.into_iter().map(String::as_str).collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Ok(HashSet::new());
    }
    Ok(CachedEntry::get_version_v3_many(
        &ids,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .into_iter()
    .filter(|version| {
        matches!(
            version.environment,
            Some(VersionEnvironment::DedicatedServerOnly)
        )
    })
    .map(|version| version.id)
    .collect())
}
