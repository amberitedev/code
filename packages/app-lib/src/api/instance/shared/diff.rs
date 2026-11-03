//! The App's share diffs: each reads both sides into a `ShareContent` and diffs them with
//! sharing-client's `share_diff`, which Core's push preview uses too.

use super::client::*;
use super::content::remote_shared_content;
use super::publish::*;
use super::types::*;
use super::*;
use sharing_client::diff::{
    DiffError, DiffOptions, Disabled, Names, ShareContent, VersionName,
    share_diff,
};
use std::collections::BTreeMap;

const CONFIG_HASH_CONCURRENCY: usize = 16;

/// Modrinth's names through the App's cache.
struct AppNames<'a>(&'a State);

impl Names for AppNames<'_> {
    type Error = crate::Error;

    async fn versions(
        &self,
        ids: &[String],
    ) -> crate::Result<Vec<VersionName>> {
        let ids = ids.iter().map(String::as_str).collect::<Vec<_>>();
        Ok(CachedEntry::get_version_many(
            &ids,
            Some(CacheBehaviour::Bypass),
            &self.0.pool,
            &self.0.api_semaphore,
        )
        .await?
        .into_iter()
        .map(|version| VersionName {
            id: version.id,
            project_id: version.project_id,
            name: version.name,
            version_number: version.version_number,
        })
        .collect())
    }

    async fn projects(
        &self,
        ids: &[String],
    ) -> crate::Result<HashMap<String, String>> {
        shared_project_names(&ids.iter().cloned().collect(), self.0).await
    }
}

async fn diff(
    before: &ShareContent,
    after: &ShareContent,
    options: &DiffOptions,
    state: &State,
) -> crate::Result<Vec<SharedInstanceUpdateDiff>> {
<<<<<<< HEAD
    share_diff(before, after, options, &AppNames(state))
        .await
        .map_err(|error| match error {
            DiffError::MissingVersion(_) => {
                crate::ErrorKind::InputError(error.to_string()).into()
            }
            DiffError::Names(error) => error,
        })
}
=======
    let before_configuration = local_configuration(metadata);
    let after_configuration = remote_configuration(version);
    let modpack_unlinked = before_configuration.modpack_version_id.is_some()
        && after_configuration.modpack_version_id.is_none();
    let is_owner = metadata
        .shared_instance
        .as_ref()
        .is_some_and(|attachment| !attachment.role.is_member());
    let (version_ids, external_files) =
        current_shared_content(metadata, modpack_unlinked, is_owner, state)
            .await?;
    let before = SharedContentSnapshot {
        version_ids,
        external_files,
        configuration: before_configuration,
    };
    let (version_ids, external_files) = remote_shared_content(version)?;
    let after = SharedContentSnapshot {
        version_ids,
        external_files,
        configuration: after_configuration,
    };
>>>>>>> modrinth/main

/// A member's update: their instance's shared content, then `version`. `configs` is what the
/// update does to their config files ([`unedited_configs`] of what left the share is removed).
pub(super) async fn shared_instance_update_diffs(
    metadata: &crate::state::InstanceMetadata,
    version: &ShareVersion,
    written: &[ManifestFile],
    removed: &[ManifestFile],
    state: &State,
) -> crate::Result<Vec<SharedInstanceUpdateDiff>> {
    let (mut before_ids, mut external_files) =
        current_shared_content(metadata, state).await?;
    let mut after = remote_shared_content(version, state).await?;
    // Players never run server-only mods, so a change to one is for the instance's servers and
    // prompts no member.
    let server_only = super::server::server_only_versions(
        before_ids.iter().chain(&after.version_ids),
        state,
    )
    .await?;
    before_ids.retain(|id| !server_only.contains(id));
    after.version_ids.retain(|id| !server_only.contains(id));
    // Upstream shows every external file in both as updated. Amberite's manifest carries each
    // file's SHA-1, so a file the member already has is left out.
    if !crate::self_hosted::accounts::enabled() {
        external_files.values_mut().for_each(|sha1| *sha1 = None);
        after
            .external_files
            .values_mut()
            .for_each(|sha1| *sha1 = None);
    }
    // Only the configs the update writes or removes are compared: the member keeps the rest.
    let current =
        configs_on_disk(&metadata.instance.id, written, state).await?;
    after.configs = sha1s(written);
    let before = ShareContent {
        version_ids: before_ids,
        external_files,
        configs: sha1s(current.into_iter().chain(removed)),
        configuration: local_configuration(metadata),
    };
    let options = DiffOptions {
        common_external_files: CommonExternalFilePolicy::AssumeUpdated,
        ..Default::default()
    };
    diff(&before, &after, &options, state).await
}

/// The configs that left the share which the member still has exactly as the share sent them,
/// so an update deletes them. A config the member edited stays theirs.
pub(super) async fn unedited_configs(
    instance_id: &str,
    left: &[ManifestFile],
    state: &State,
) -> crate::Result<Vec<ManifestFile>> {
    Ok(configs_on_disk(instance_id, left, state)
        .await?
        .into_iter()
        .cloned()
        .collect())
}

fn sha1s<'a>(
    files: impl IntoIterator<Item = &'a ManifestFile>,
) -> BTreeMap<String, String> {
    files
        .into_iter()
        .map(|file| (file.file_name.clone(), file.sha1.clone()))
        .collect()
}

/// The config files the instance has byte for byte as listed, by size and SHA-1. One that cannot
/// be checked counts as different.
async fn configs_on_disk<'a>(
    instance_id: &str,
    files: &'a [ManifestFile],
    state: &State,
) -> crate::Result<Vec<&'a ManifestFile>> {
    use futures::{StreamExt, future::BoxFuture};

    let files = files
        .iter()
        .filter(|file| file.file_type == CONFIG_FILE_TYPE)
        .collect::<Vec<_>>();
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let writer =
        crate::api::instance::InstanceFileWriter::new(state, instance_id)
            .await?;
    let writer = &writer;
    // Boxed as Send first: a stream of borrowing closures makes the Tauri command's future fail
    // the higher-ranked Send check.
    let checks = files
        .into_iter()
        .map(|file| -> BoxFuture<'_, Option<&'a ManifestFile>> {
            Box::pin(async move {
                crate::install::instance_has_shared_file(
                    writer,
                    state,
                    &file.file_name,
                    file.file_size,
                    Some(&file.sha1),
                )
                .await
                .unwrap_or(false)
                .then_some(file)
            })
        })
        .collect::<Vec<_>>();
    Ok(futures::stream::iter(checks)
        .buffer_unordered(CONFIG_HASH_CONCURRENCY)
        .filter_map(std::future::ready)
        .collect()
        .await)
}

/// What `after` changed over `before`, two versions of the same share, for its version history,
/// with what `after` holds. Without `before`, everything `after` holds is added.
pub(super) async fn shared_version_diffs(
    before: Option<&ShareVersion>,
    after: &ShareVersion,
    state: &State,
) -> crate::Result<(Vec<SharedInstanceUpdateDiff>, ShareContent)> {
    let after = remote_shared_content(after, state).await?;
    let before = match before {
        Some(before) => remote_shared_content(before, state).await?,
        None => ShareContent::empty(after.configuration.clone()),
    };
    let diffs = diff(&before, &after, &DiffOptions::default(), state).await?;
    Ok((diffs, after))
}

/// An owner's push: `version`, the latest, then what a push of the instance sends now.
pub(super) async fn shared_instance_publish_diffs(
    metadata: &crate::state::InstanceMetadata,
    version: &ShareVersion,
    snapshot: &CurrentPublishSnapshot,
    state: &State,
) -> crate::Result<Vec<SharedInstanceUpdateDiff>> {
    let before = remote_shared_content(version, state).await?;
    let configuration = local_configuration(metadata);
    let instance_path = state
        .directories
        .instances_dir()
        .join(&metadata.instance.path);
    let mut external_files = BTreeMap::new();
    for file in &snapshot.external_files {
        let key = shared_external_file_key(&file.file_type, &file.file_name)?;
        let sha1 = match &file.source {
            ExternalFileSource::InstanceFile(path)
                if crate::self_hosted::accounts::enabled() =>
            {
                // One that cannot be read counts as changed.
                Some(
                    crate::util::fetch::sha1_file_async(
                        instance_path.join(path),
                    )
                    .await
                    .map_or_else(|_| String::new(), |(_, sha1)| sha1),
                )
            }
            _ => None,
        };
        external_files.insert(key, sha1);
    }
    // A push sends every config still shared as the instance has it, so one the owner edited or
    // deleted is a change to review, as on Core.
    let shared = version.configs().cloned().collect::<Vec<_>>();
    let current =
        configs_on_disk(&metadata.instance.id, &shared, state).await?;
    let after = ShareContent {
        version_ids: snapshot
            .effective_version_ids
            .iter()
            .filter(|id| {
                configuration.modpack_version_id.as_deref() != Some(id.as_str())
            })
            .cloned()
            .collect(),
        external_files,
        configs: sha1s(current),
        configuration,
    };
    let options = DiffOptions {
        common_external_files: CommonExternalFilePolicy::AssumeUnchanged,
        disabled: Disabled {
            project_ids: snapshot.disabled_project_ids.clone(),
            version_ids: snapshot
                .disabled_version_ids
                .iter()
                .cloned()
                .collect(),
            external_files: snapshot.disabled_external_files.clone(),
        },
    };
    diff(&before, &after, &options, state).await
}

fn local_configuration(
    metadata: &crate::state::InstanceMetadata,
) -> ContentSetConfiguration {
    ContentSetConfiguration {
        modpack_version_id: shared_modpack_id(&metadata.link)
            .filter(|id| !id.is_empty()),
        game_version: metadata.applied_content_set.game_version.clone(),
        loader: LoaderReference {
            name: metadata.applied_content_set.loader.as_str().to_string(),
            version: metadata.applied_content_set.loader_version.clone(),
        },
    }
}

pub(super) fn remote_configuration(
    version: &ShareVersion,
) -> ContentSetConfiguration {
    let (loader, loader_version) = instance_loader(version);
    ContentSetConfiguration {
        modpack_version_id: version.modpack_id().map(str::to_owned),
        game_version: version.content.game_version.clone(),
        loader: LoaderReference {
            name: loader.as_str().to_string(),
            version: Some(loader_version),
        },
    }
}

pub(super) fn shared_external_file_key(
    file_type: &str,
    path: &str,
) -> crate::Result<ExternalFileKey> {
    Ok(ExternalFileKey {
        content_type: file_type.parse().map_err(
            |error: modrinth_content_management::Error| {
                crate::ErrorKind::InputError(error.to_string())
            },
        )?,
        path: path.to_string(),
    })
}
