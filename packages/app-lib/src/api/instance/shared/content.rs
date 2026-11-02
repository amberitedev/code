use super::client::{ShareVersion, check_removed_files, get_remote_version};
use super::diff::remote_configuration;
use super::*;
use crate::api::pack::install_from::{
    EnvType, PackDependency, PackFileHash, PackFormat,
};
use crate::state::{DependencyType, SideType};
use crate::util::fetch::{DownloadMeta, DownloadReason};
use sharing_client::diff::{CarriedFile, ShareContent};
use sharing_client::install::{ConfigUpdate, ModpackFile, config_update};
use std::collections::BTreeMap;

/// What reaching `latest` does to the member's config files (see
/// `sharing_client::install::config_update`), from the version they applied. Every config is
/// written on a first install, or when the backend no longer keeps the applied version.
pub(super) async fn remote_config_update(
    shared_instance_id: &str,
    applied_version: Option<i32>,
    latest: &ShareVersion,
    state: &State,
) -> crate::Result<ConfigUpdate> {
    let applied = match applied_version {
        Some(version) if version >= latest.version => {
            return Ok(ConfigUpdate::default());
        }
        Some(version) => {
            get_remote_version(shared_instance_id, version, state).await?
        }
        None => None,
    };
    // A changed `backtrack_version` starts the member over, so every config is written.
    let applied = applied.filter(|applied| {
        applied.backtrack_version == latest.backtrack_version
    });
    Ok(config_update(
        &latest.external_files,
        applied
            .as_ref()
            .map(|version| version.external_files.as_slice()),
    ))
}

/// Reads the client content from the cached pack archive, including unresolved overrides.
#[tracing::instrument(skip(state))]
pub(crate) async fn shared_modpack_files(
    modpack_id: &str,
    state: &State,
) -> crate::Result<Vec<ModpackFile>> {
    let version = CachedEntry::get_version(
        modpack_id,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or(crate::ErrorKind::InputError(
        "shared instance modpack version was not found".to_string(),
    ))?;
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or(crate::ErrorKind::InputError(
            "shared instance modpack has no files".to_string(),
        ))?;
    let download_meta = DownloadMeta {
        reason: DownloadReason::Modpack,
        game_version: version
            .game_versions
            .first()
            .cloned()
            .unwrap_or_default(),
        loader: version.loaders.first().cloned().unwrap_or_default(),
        dependent_on: Some(modpack_id.to_string()),
    };
    let archive = crate::util::fetch::fetch_content_file(
        state,
        &[&file.url],
        file.hashes.get("sha512").map(String::as_str),
        Some(u64::from(file.size)),
        Some(&download_meta),
        None,
    )
    .await?;
    let archive_path = archive.path().to_path_buf();
    let (manifest, override_hashes) =
        tokio::task::spawn_blocking(move || -> crate::Result<_> {
            use std::io::Read;

            let mut zip = zip::ZipArchive::new(std::io::BufReader::new(
                std::fs::File::open(archive_path)?,
            ))?;
            let mut manifest_index = None;
            let mut overrides = Vec::new();
            for index in 0..zip.len() {
                let entry = zip.by_index_raw(index)?;
                let Ok(filename) = entry.name() else {
                    continue;
                };
                if filename == "modrinth.index.json" {
                    manifest_index.get_or_insert(index);
                } else if let Some(path) = filename
                    .strip_prefix("overrides/")
                    .or_else(|| filename.strip_prefix("client-overrides/"))
                    && is_content_path(path)
                {
                    overrides.push((index, path.to_string()));
                }
            }
            let manifest_index =
                manifest_index.ok_or(crate::ErrorKind::InputError(
                    "no modrinth.index.json found in shared modpack"
                        .to_string(),
                ))?;
            let mut manifest = String::new();
            zip.by_index(manifest_index)?
                .read_to_string(&mut manifest)?;
            let mut buffer = vec![0_u8; 256 * 1024];
            let mut override_hashes = Vec::with_capacity(overrides.len());
            for (index, path) in overrides {
                let mut reader = zip.by_index(index)?;
                let mut hash = sha1_smol::Sha1::new();
                loop {
                    let read = reader.read(&mut buffer)?;
                    if read == 0 {
                        break;
                    }
                    hash.update(&buffer[..read]);
                }
                override_hashes.push((path, hash.hexdigest()));
            }
            Ok((manifest, override_hashes))
        })
        .await??;
    let pack: PackFormat = serde_json::from_str(&manifest)?;
    let default_parent = if pack
        .dependencies
        .keys()
        .any(|loader| *loader != PackDependency::Minecraft)
    {
        "mods"
    } else {
        "datapacks"
    };
    let unresolved_basenames = version
        .dependencies
        .iter()
        .filter(|dependency| {
            matches!(dependency.dependency_type, DependencyType::Embedded)
                && dependency.version_id.is_none()
        })
        .filter_map(|dependency| dependency.file_name.as_deref())
        .filter(|filename| !filename.contains('/'))
        .collect::<HashSet<_>>();
    let mut files = BTreeMap::new();
    for file in pack.files {
        if file.env.as_ref().is_some_and(|env| {
            env.get(&EnvType::Client) == Some(&SideType::Unsupported)
        }) {
            continue;
        }
        let path = file.path.as_str();
        if is_content_path(path) {
            files.insert(
                path.to_string(),
                file.hashes.get(&PackFileHash::Sha1).cloned(),
            );
        }
    }
    for (path, hash) in override_hashes {
        files.insert(path, Some(hash));
    }
    let hashes = files
        .values()
        .filter_map(|hash| hash.as_deref())
        .collect::<Vec<_>>();
    let metadata = CachedEntry::get_file_many(
        &hashes,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .into_iter()
    .map(|file| (file.hash.clone(), file))
    .collect::<HashMap<_, _>>();
    Ok(files
        .into_iter()
        .map(|(relative_path, hash)| {
            let metadata = hash.as_ref().and_then(|hash| metadata.get(hash));
            // Hosting uses the loader's default directory when Labrinth supplies only an unresolved basename.
            let removed_path_alias =
                relative_path.rsplit_once('/').and_then(|(_, filename)| {
                    unresolved_basenames
                        .contains(filename)
                        .then(|| format!("{default_parent}/{filename}"))
                });
            ModpackFile {
                path: relative_path,
                sha1: hash,
                version_id: metadata.map(|file| file.version_id.clone()),
                alias: removed_path_alias,
                source: (),
            }
        })
        .collect())
}

fn is_content_path(path: &str) -> bool {
    crate::state::content_store::is_managed_content_path(path)
        && !path.ends_with(".disabled")
}

/// The share content `version` gives members, as the share diff reads it.
pub(super) async fn remote_shared_content(
    version: &ShareVersion,
    state: &State,
) -> crate::Result<ShareContent> {
    check_removed_files(version)?;
    let modpack = match version.modpack_id() {
        Some(modpack_id) => shared_modpack_files(modpack_id, state).await?,
        None => Vec::new(),
    };
    ShareContent::of(
        &version.content,
        version.external_files.iter().map(CarriedFile::from),
        modpack,
        remote_configuration(version),
    )
    .map_err(|error| crate::ErrorKind::InputError(error.to_string()).into())
}
