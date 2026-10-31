use super::client::{
    ExternalFileResponse, InstanceVersionResponse, get_remote_version,
};
use super::diff::shared_external_file_key;
use super::publish::dedupe_strings;
use super::*;
use crate::api::pack::install_from::{
    EnvType, PackDependency, PackFileHash, PackFormat,
};
use crate::state::{DependencyType, SideType};
use crate::util::fetch::{DownloadMeta, DownloadReason};
use std::collections::BTreeMap;

/// The config files a member writes to reach `latest`. Every version lists all config files
/// shared so far, so only those that changed since `applied_version` are written and the rest
/// stay as the member has them, as when upstream published configs as deltas. Every config is
/// written on a first install, or when the backend no longer keeps the applied version.
pub(super) async fn remote_shared_config_files(
    shared_instance_id: &str,
    applied_version: Option<i32>,
    latest: &InstanceVersionResponse,
    state: &State,
) -> crate::Result<Vec<ExternalFileResponse>> {
    let applied = match applied_version {
        Some(version) if version >= latest.version => return Ok(Vec::new()),
        Some(version) => {
            get_remote_version(shared_instance_id, version, state).await?
        }
        None => None,
    };
    // A changed `backtrack_version` starts the member over, so every config is written.
    let applied = applied.filter(|applied| {
        applied.backtrack_version == latest.backtrack_version
    });
    changed_config_files(
        &latest.external_files,
        applied
            .as_ref()
            .map(|version| version.external_files.as_slice()),
    )
}

/// The config files of `latest` that are new or differ from `applied`. A legacy config bundle
/// comes first so individual files are written over it.
pub(super) fn changed_config_files(
    latest: &[ExternalFileResponse],
    applied: Option<&[ExternalFileResponse]>,
) -> crate::Result<Vec<ExternalFileResponse>> {
    ensure_config_limits(latest)?;
    let applied = applied
        .unwrap_or_default()
        .iter()
        .map(|file| ((&file.file_type, &file.file_name), &file.sha256))
        .collect::<HashMap<_, _>>();
    let mut bundle = None;
    let mut files = BTreeMap::new();
    for file in latest.iter().filter(|file| {
        applied.get(&(&file.file_type, &file.file_name)) != Some(&&file.sha256)
    }) {
        match file.file_type.as_str() {
            CONFIG_BUNDLE_FILE_TYPE => bundle = Some(file.clone()),
            CONFIG_FILE_TYPE => {
                files.insert(file.file_name.clone(), file.clone());
            }
            _ => {}
        }
    }

    Ok(bundle.into_iter().chain(files.into_values()).collect())
}

fn ensure_config_limits(
    external_files: &[ExternalFileResponse],
) -> crate::Result<()> {
    let mut count = 0;
    let mut size = 0_u64;
    for file in external_files
        .iter()
        .filter(|file| file.file_type == CONFIG_FILE_TYPE)
    {
        count += 1;
        size = size.saturating_add(
            file.file_size
                .and_then(|size| u64::try_from(size).ok())
                .unwrap_or(u64::MAX),
        );
    }
    if count > MAX_SHARED_FILES || size > MAX_SHARED_FILES_TOTAL_SIZE {
        bail!(crate::ErrorKind::InputError(
            "shared instance files exceed the size or file count limit"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) struct SharedModpackFile {
    pub relative_path: String,
    pub sha1: Option<String>,
    pub version_id: Option<String>,
    removed_path_alias: Option<String>,
}

impl SharedModpackFile {
    pub(crate) fn is_removed(
        &self,
        removed_files: &[SharedInstanceRemovedFile],
    ) -> bool {
        removed_files.iter().any(|removed| {
            removed.matches(self.version_id.as_deref(), &self.relative_path)
                || self
                    .removed_path_alias
                    .as_deref()
                    .is_some_and(|path| removed.matches(None, path))
        })
    }
}

/// Reads the client content from the cached pack archive, including unresolved overrides.
#[tracing::instrument(skip(state))]
pub(crate) async fn shared_modpack_files(
    modpack_id: &str,
    state: &State,
) -> crate::Result<Vec<SharedModpackFile>> {
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
            SharedModpackFile {
                relative_path,
                sha1: hash,
                version_id: metadata.map(|file| file.version_id.clone()),
                removed_path_alias,
            }
        })
        .collect())
}

fn is_content_path(path: &str) -> bool {
    crate::state::content_store::is_managed_content_path(path)
        && !path.ends_with(".disabled")
}

pub(super) async fn remote_shared_content(
    version: &InstanceVersionResponse,
    state: &State,
) -> crate::Result<(Vec<String>, BTreeSet<ExternalFileKey>)> {
    version.validate_removed_files()?;
    let mut version_ids = version
        .modrinth_ids
        .iter()
        .filter(|id| version.modpack_id.as_deref() != Some(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let mut external_files = version
        .external_files
        .iter()
        .filter(|file| {
            !matches!(
                file.file_type.as_str(),
                CONFIG_BUNDLE_FILE_TYPE | CONFIG_FILE_TYPE
            )
        })
        .map(|file| shared_external_file_key(&file.file_type, &file.file_name))
        .collect::<crate::Result<BTreeSet<_>>>()?;
    let explicit_paths = version
        .external_files
        .iter()
        .filter_map(|file| {
            ProjectType::from_name(&file.file_type)
                .map(|kind| format!("{}/{}", kind.get_folder(), file.file_name))
        })
        .collect::<HashSet<_>>();
    if let Some(modpack_id) =
        version.modpack_id.as_deref().filter(|id| !id.is_empty())
    {
        for file in shared_modpack_files(modpack_id, state).await? {
            if file.is_removed(&version.removed_files)
                || explicit_paths.contains(&file.relative_path)
            {
                continue;
            }
            if let Some(version_id) = file.version_id {
                version_ids.push(version_id);
            } else if let Some((parent, filename)) =
                file.relative_path.split_once('/')
            {
                external_files.insert(ExternalFileKey {
                    content_type: ProjectType::from_name(parent)
                        .expect("pack content path was validated")
                        .into(),
                    path: filename.to_string(),
                });
            }
        }
    }
    dedupe_strings(&mut version_ids);
    Ok((version_ids, external_files))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(
        file_type: &str,
        file_name: &str,
        sha256: &str,
    ) -> ExternalFileResponse {
        ExternalFileResponse {
            file_name: file_name.to_string(),
            file_type: file_type.to_string(),
            file_size: Some(1),
            sha256: Some(sha256.to_string()),
            ..Default::default()
        }
    }

    fn names(files: &[ExternalFileResponse]) -> Vec<(&str, &str)> {
        files
            .iter()
            .map(|file| (file.file_type.as_str(), file.file_name.as_str()))
            .collect()
    }

    #[test]
    fn only_configs_changed_since_the_applied_version_are_written() {
        let applied = [
            file(CONFIG_FILE_TYPE, "config/same.toml", "1"),
            file(CONFIG_FILE_TYPE, "config/changed.toml", "1"),
        ];
        let latest = [
            file("mod", "extra.jar", "2"),
            file(CONFIG_FILE_TYPE, "config/same.toml", "1"),
            file(CONFIG_FILE_TYPE, "config/changed.toml", "2"),
            file(CONFIG_FILE_TYPE, "config/new.toml", "2"),
        ];

        let files = changed_config_files(&latest, Some(&applied)).unwrap();

        assert_eq!(
            names(&files),
            [
                (CONFIG_FILE_TYPE, "config/changed.toml"),
                (CONFIG_FILE_TYPE, "config/new.toml"),
            ]
        );
    }

    #[test]
    fn a_first_install_writes_every_config_with_the_bundle_first() {
        let latest = [
            file(CONFIG_FILE_TYPE, "config/a.toml", "1"),
            file(CONFIG_BUNDLE_FILE_TYPE, "configs.zip", "1"),
        ];

        let files = changed_config_files(&latest, None).unwrap();

        assert_eq!(
            names(&files),
            [
                (CONFIG_BUNDLE_FILE_TYPE, "configs.zip"),
                (CONFIG_FILE_TYPE, "config/a.toml"),
            ]
        );
    }

    #[test]
    fn limits_apply_to_the_version() {
        let mut half = file(CONFIG_FILE_TYPE, "config/a.toml", "1");
        half.file_size = Some((MAX_SHARED_FILES_TOTAL_SIZE / 2 + 1) as i64);
        let mut other_half = half.clone();
        other_half.file_name = "config/b.toml".to_string();

        assert!(changed_config_files(&[half.clone()], None).is_ok());
        assert!(changed_config_files(&[half, other_half], None).is_err());
    }
}
