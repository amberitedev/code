use super::client::*;
use super::diff::*;
use super::install::*;
use super::report::{PublishReport, PublishReportFile};
use super::types::*;
use super::*;
use crate::self_hosted::pool::{self, crypto};
use async_walkdir::{Filtering, WalkDir};
use futures::{StreamExt, TryStreamExt};
use sha2::Digest;
use std::time::Duration;

/// Files hashed or encrypted at once.
const UPLOAD_CONCURRENCY: usize = 4;
/// The Core confirms a stored version to the backend on its own, so a push waits up to a minute.
const CONFIRM_INTERVAL: Duration = Duration::from_secs(2);
const CONFIRM_ATTEMPTS: usize = 30;

#[tracing::instrument]
pub async fn unpublish_shared_instance(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let _shared_instance_lock =
        state.instance_locks.lock_shared(instance_id).await;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let Some(attachment) = metadata.shared_instance.clone() else {
        return Ok(());
    };
    ensure_owner(&attachment)?;

    delete_remote_instance(&attachment.id, &state).await?;
    detach_local_shared_instance(instance_id, metadata.link, &state).await?;
    emit_instance(instance_id, InstancePayloadType::Edited).await?;

    Ok(())
}

#[tracing::instrument]
pub async fn unlink_shared_instance(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let _shared_instance_lock =
        state.instance_locks.lock_shared(instance_id).await;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let Some(attachment) = metadata.shared_instance.clone() else {
        return Ok(());
    };
    ensure_member(&attachment)?;

    detach_local_shared_instance(instance_id, metadata.link, &state).await?;
    emit_instance(instance_id, InstancePayloadType::Edited).await?;

    Ok(())
}

pub(super) async fn detach_local_shared_instance(
    instance_id: &str,
    link: InstanceLink,
    state: &State,
) -> crate::Result<()> {
    let link_patch = match link {
        InstanceLink::SharedInstance {
            modpack_project_id: Some(project_id),
            modpack_version_id: Some(version_id),
        } => Some((
            InstanceLink::ModrinthModpack {
                project_id,
                version_id,
            },
            ContentSourceKind::ModrinthModpack,
        )),
        InstanceLink::SharedInstance { .. } => {
            Some((InstanceLink::Unmanaged, ContentSourceKind::Local))
        }
        _ => None,
    };

    if let Some((link, source_kind)) = link_patch {
        crate::state::edit_instance(
            instance_id,
            EditInstance {
                link: Some(link),
                content_set_patch: Some(AppliedContentSetPatch {
                    source_kind: Some(source_kind),
                    ..Default::default()
                }),
                ..Default::default()
            },
            state,
        )
        .await?;
    }

    crate::state::clear_shared_instance(instance_id, &state.pool).await?;

    Ok(())
}

/// `on_progress` receives the uploaded and total bytes of the files the
/// shared instances service asks for. `server_core` is a Core the owner owns
/// that runs a linked server of this instance; the backend keeps the share's
/// home copy there.
#[tracing::instrument(skip(on_progress))]
pub async fn publish_shared_instance(
    instance_id: &str,
    file_paths: Vec<String>,
    server_core: Option<String>,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
) -> crate::Result<SharedInstanceAttachment> {
    let state = State::get().await?;
    publish_shared_instance_inner(
        instance_id,
        &file_paths,
        server_core.as_deref(),
        on_progress,
        &state,
    )
    .await?;
    emit_instance(instance_id, InstancePayloadType::Edited).await?;

    shared_attachment(instance_id, &state).await?.ok_or(
        crate::ErrorKind::InputError(
            "shared instance attachment was not persisted".to_string(),
        )
        .into(),
    )
}

#[tracing::instrument]
pub async fn get_shared_instance_publish_preview(
    instance_id: &str,
) -> crate::Result<Option<SharedInstancePublishPreview>> {
    let state = State::get().await?;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let Some(attachment) = metadata.shared_instance.clone() else {
        return Ok(None);
    };
    ensure_owner(&attachment)?;

    let (version, snapshot) = tokio::try_join!(
        get_latest_remote_version_optional_unavailable(&attachment.id, &state,),
        collect_publish_snapshot(&metadata, &state),
    )?;
    let version = match version {
        SharedInstanceRemoteResponse::Available(version) => version,
        SharedInstanceRemoteResponse::Unavailable(reason) => {
            handle_unavailable_shared_instance_if_current_user(
                instance_id,
                &attachment,
                reason,
                &state,
            )
            .await?;
            return Err(shared_instance_unavailable_error(reason));
        }
    };
    let diffs =
        shared_instance_publish_diffs(&metadata, &version, &snapshot, &state)
            .await?;
    set_shared_instance_publish_status(
        instance_id,
        &attachment,
        version.version,
        !diffs.is_empty(),
        &state,
    )
    .await?;
    emit_instance(instance_id, InstancePayloadType::Edited).await?;

    Ok(Some(SharedInstancePublishPreview {
        shared_instance_id: attachment.id,
        latest_version: version.version,
        diffs,
        files: snapshot.shared_files,
    }))
}

pub(super) async fn shared_instance_install_modpack(
    version: &InstanceVersionResponse,
    state: &State,
) -> crate::Result<Option<crate::install::SharedInstanceInstallModpack>> {
    let Some(modpack_id) =
        version.modpack_id.as_deref().filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    let modpack_version = CachedEntry::get_version(
        modpack_id,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or(crate::ErrorKind::InputError(
        "shared instance modpack version was not found".to_string(),
    ))?;
    let project = CachedEntry::get_project(
        &modpack_version.project_id,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?;

    Ok(Some(crate::install::SharedInstanceInstallModpack {
        project_id: modpack_version.project_id,
        version_id: modpack_version.id,
        title: project
            .as_ref()
            .map(|project| project.title.clone())
            .unwrap_or(modpack_version.name),
        icon_url: project.and_then(|project| project.raw_icon_url),
    }))
}

pub(super) fn shared_instance_loader_version(
    loader_version: String,
) -> Option<String> {
    (!loader_version.is_empty()).then_some(loader_version)
}

/// The shared content installed now: Modrinth version ids, external files, and the SHA-1 of each
/// external file present on disk, by name and file type.
pub(super) async fn current_shared_content(
    metadata: &crate::state::InstanceMetadata,
    state: &State,
) -> crate::Result<(
    Vec<String>,
    BTreeSet<ExternalFileKey>,
    HashMap<(String, String), String>,
)> {
    let entries = crate::state::instances::adapters::sqlite::content_rows::get_content_entries(
		&metadata.applied_content_set.id, &state.pool,
	).await?;
    let files = crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
		&metadata.instance.id, &state.pool,
	).await?.into_iter().map(|file| (file.id.clone(), file)).collect::<HashMap<_, _>>();
    let unresolved_hashes = entries
        .iter()
        .filter(|entry| {
            entry.source_kind == ContentSourceKind::ModrinthModpack
                && entry.version_id.is_none()
        })
        .filter_map(|entry| entry.file_id.as_ref().and_then(|id| files.get(id)))
        .filter(|file| file.enabled && !file.missing)
        .map(|file| file.sha1.as_str())
        .collect::<Vec<_>>();
    let resolved_files = CachedEntry::get_file_many(
        &unresolved_hashes,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .into_iter()
    .map(|file| (file.hash.clone(), file))
    .collect::<HashMap<_, _>>();
    let mut version_ids = Vec::new();
    let mut external_files = BTreeSet::new();
    let mut external_file_sha1s = HashMap::new();
    for entry in entries {
        if !entry.source_kind.is_shared_instance_managed() || !entry.enabled {
            continue;
        }
        let Some(file) = entry.file_id.as_ref().and_then(|id| files.get(id))
        else {
            continue;
        };
        if !file.enabled || file.missing {
            continue;
        }
        let version_id = entry.version_id.or_else(|| {
            (entry.source_kind == ContentSourceKind::ModrinthModpack)
                .then(|| {
                    resolved_files
                        .get(&file.sha1)
                        .map(|file| file.version_id.clone())
                })
                .flatten()
        });
        if let Some(version_id) = version_id {
            version_ids.push(version_id);
        } else {
            let path = enabled_file_name(&file.file_name);
            external_file_sha1s.insert(
                (path.clone(), file_type(entry.project_type)),
                file.sha1.clone(),
            );
            external_files.insert(ExternalFileKey {
                content_type: entry.project_type.into(),
                path,
            });
        }
    }
    dedupe_strings(&mut version_ids);
    Ok((version_ids, external_files, external_file_sha1s))
}

pub(super) struct CurrentPublishSnapshot {
    pub(super) version_ids: Vec<String>,
    pub(super) external_files: Vec<ExternalFileCandidate>,
    pub(super) disabled_project_ids: HashSet<String>,
    pub(super) disabled_version_ids: Vec<String>,
    pub(super) disabled_external_files: BTreeSet<ExternalFileKey>,
    pub(super) shared_files: Vec<SharedInstancePublishFile>,
    pub(super) removed_files: Vec<SharedInstanceRemovedFile>,
    pub(super) effective_version_ids: Vec<String>,
}

/// Cached metadata can outlive a deleted or hidden version. Only publish version
/// IDs that other members can resolve; retain installed files as uploads.
pub(super) async fn collect_publish_snapshot(
    metadata: &crate::state::InstanceMetadata,
    state: &State,
) -> crate::Result<CurrentPublishSnapshot> {
    let instance_path = state
        .directories
        .instances_dir()
        .join(&metadata.instance.path);
    let (items, shared_files) = if CONFIG_SYNC_ENABLED {
        tokio::try_join!(
            crate::state::list_content(
                &metadata.instance.id,
                None,
                None,
                state,
            ),
            collect_shareable_files(&instance_path),
        )?
    } else {
        (
            crate::state::list_content(
                &metadata.instance.id,
                None,
                None,
                state,
            )
            .await?,
            Vec::new(),
        )
    };
    let mut items = items;
    let modpack_id = shared_modpack_id(&metadata.link);
    let modpack_files = if let Some(modpack_id) = modpack_id.as_deref() {
        let (managed, files) = tokio::try_join!(
            crate::state::list_linked_modpack_content(
                &metadata.instance.id,
                None,
                None,
                state,
            ),
            shared_modpack_files(modpack_id, state),
        )?;
        items.extend(managed);
        files
    } else {
        Vec::new()
    };
    let mut seen_paths = HashSet::new();
    items.retain(|item| seen_paths.insert(item.file_path.clone()));
    let enabled_versions = items
        .iter()
        .filter(|item| item.enabled)
        .filter_map(|item| {
            item.version.as_ref().map(|version| version.id.as_str())
        })
        .collect::<HashSet<_>>();
    let enabled_paths = items
        .iter()
        .filter(|item| item.enabled)
        .map(|item| item.file_path.trim_end_matches(".disabled"))
        .collect::<HashSet<_>>();
    let mut removed_files = Vec::new();
    for file in &modpack_files {
        if let Some(version_id) = &file.version_id {
            if !enabled_versions.contains(version_id.as_str()) {
                let removed = SharedInstanceRemovedFile::Version {
                    version_id: version_id.clone(),
                };
                if !removed_files.contains(&removed) {
                    removed_files.push(removed);
                }
            }
        } else if !enabled_paths.contains(file.relative_path.as_str())
            && let Some((parent, filename)) = file.relative_path.split_once('/')
        {
            removed_files.push(SharedInstanceRemovedFile::Path {
                parent: parent.to_string(),
                filename: filename.to_string(),
            });
        }
    }
    let inherited_versions = modpack_files
        .iter()
        .filter_map(|file| file.version_id.as_deref())
        .collect::<HashSet<_>>();
    let mut effective_version_ids = Vec::new();
    let installed_version_ids = items
        .iter()
        .filter(|item| item.enabled)
        .filter_map(|item| {
            item.version.as_ref().map(|version| version.id.as_str())
        })
        .collect::<Vec<_>>();
    let available_version_ids = CachedEntry::get_version_many(
        &installed_version_ids,
        Some(CacheBehaviour::Bypass),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .into_iter()
    .map(|version| version.id)
    .collect::<HashSet<_>>();
    let mut version_ids = Vec::new();
    let mut seen_version_ids = HashSet::new();
    let mut external_files = Vec::new();
    let mut seen_external_files = HashSet::new();
    let mut disabled_project_ids = HashSet::new();
    let mut disabled_version_ids = Vec::new();
    let mut seen_disabled_version_ids = HashSet::new();
    let mut disabled_external_files = BTreeSet::new();

    for item in items {
        if item.enabled {
            if let Some(version) = item.version.as_ref()
                && available_version_ids.contains(&version.id)
            {
                if seen_version_ids.insert(version.id.clone()) {
                    effective_version_ids.push(version.id.clone());
                    if !inherited_versions.contains(version.id.as_str()) {
                        version_ids.push(version.id.clone());
                    }
                }
                continue;
            }

            if item.file_path.is_empty() {
                continue;
            }

            let file_type = file_type(item.project_type);
            let external_key = format!("{}:{file_type}", item.file_path);
            if seen_external_files.insert(external_key) {
                external_files.push(ExternalFileCandidate {
                    file_name: item.file_name,
                    file_type,
                    source: ExternalFileSource::InstanceFile(item.file_path),
                });
            }
            continue;
        }

        let is_modpack = item.version.as_ref().is_some_and(|version| {
            modpack_id.as_deref() == Some(version.id.as_str())
        });
        if is_modpack {
            continue;
        }

        if item.version.is_none()
            && let Some(project) = item.project.as_ref()
        {
            disabled_project_ids.insert(project.id.clone());
        }

        if let Some(version) = item.version {
            if seen_disabled_version_ids.insert(version.id.clone()) {
                disabled_version_ids.push(version.id);
            }
            continue;
        }

        if item.file_path.is_empty() {
            continue;
        }

        disabled_external_files.insert(ExternalFileKey {
            content_type: item.project_type.into(),
            path: enabled_file_name(&item.file_name),
        });
    }

    Ok(CurrentPublishSnapshot {
        version_ids,
        external_files,
        disabled_project_ids,
        disabled_version_ids,
        disabled_external_files,
        shared_files,
        removed_files,
        effective_version_ids,
    })
}

pub(super) async fn shared_versions_by_id(
    version_ids: &[String],
    allow_missing: bool,
    state: &State,
) -> crate::Result<HashMap<String, crate::state::Version>> {
    let version_id_refs =
        version_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let versions = CachedEntry::get_version_many(
        &version_id_refs,
        Some(CacheBehaviour::Bypass),
        &state.pool,
        &state.api_semaphore,
    )
    .await?;

    let fetched_ids = versions
        .iter()
        .map(|version| version.id.as_str())
        .collect::<HashSet<_>>();
    if !allow_missing
        && let Some(missing) = version_ids
            .iter()
            .find(|id| !fetched_ids.contains(id.as_str()))
    {
        bail!(crate::ErrorKind::InputError(format!(
            "shared content version {missing} was not found"
        )));
    }
    Ok(versions
        .into_iter()
        .map(|version| (version.id.clone(), version))
        .collect())
}

pub(super) async fn shared_project_names(
    project_ids: &HashSet<String>,
    state: &State,
) -> crate::Result<HashMap<String, String>> {
    let project_id_refs =
        project_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let projects = CachedEntry::get_project_many(
        &project_id_refs,
        Some(CacheBehaviour::Bypass),
        &state.pool,
        &state.api_semaphore,
    )
    .await?;

    Ok(projects
        .into_iter()
        .map(|project| (project.id, project.title))
        .collect())
}

pub(super) fn dedupe_strings(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

pub(super) fn enabled_file_name(file_name: &str) -> String {
    file_name
        .strip_suffix(".disabled")
        .unwrap_or(file_name)
        .to_string()
}

pub(super) fn shared_instance_name(name: String) -> String {
    match name.trim() {
        "" => "Shared instance".to_string(),
        name => name.to_string(),
    }
}

pub(super) async fn set_shared_instance_publish_status(
    instance_id: &str,
    attachment: &SharedInstanceAttachment,
    latest_version: i32,
    has_changes: bool,
    state: &State,
) -> crate::Result<()> {
    let (status, applied_version) = if has_changes {
        (ContentSetSyncStatus::Stale, attachment.applied_version)
    } else {
        (ContentSetSyncStatus::UpToDate, Some(latest_version))
    };

    crate::state::set_shared_instance_sync_status(
        instance_id,
        status,
        applied_version,
        Some(latest_version),
        &state.pool,
    )
    .await
}

pub(super) async fn publish_shared_instance_inner(
    instance_id: &str,
    file_paths: &[String],
    server_core: Option<&str>,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
    state: &State,
) -> crate::Result<()> {
    let mut report = PublishReport::new(instance_id, file_paths);
    let result = publish_shared_instance_reported(
        instance_id,
        file_paths,
        server_core,
        on_progress,
        &mut report,
        state,
    )
    .await;
    report.finish(&result);
    result
}

async fn publish_shared_instance_reported(
    instance_id: &str,
    file_paths: &[String],
    server_core: Option<&str>,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
    report: &mut PublishReport,
    state: &State,
) -> crate::Result<()> {
    let publishing_account = linked_amberite_user_id(state).await?;
    let attachment = shared_attachment(instance_id, state).await?.ok_or(
        crate::ErrorKind::InputError(
            "instance is not attached to a shared instance".to_string(),
        ),
    )?;
    report.shared_instance_id = Some(attachment.id.clone());
    report.applied_version = attachment.applied_version;
    report.latest_version = attachment.latest_version;
    ensure_owner(&attachment)?;
    tracing::info!(
        instance_id,
        shared_instance_id = %attachment.id,
        applied_version = attachment.applied_version,
        latest_version = attachment.latest_version,
        "Publishing shared instance content"
    );

    crate::state::set_shared_instance_sync_status(
        instance_id,
        ContentSetSyncStatus::Applying,
        attachment.applied_version,
        attachment.latest_version,
        &state.pool,
    )
    .await?;

    let result = publish_current_content(
        instance_id,
        &attachment.id,
        file_paths,
        server_core,
        on_progress,
        report,
        state,
    )
    .await;

    if crate::self_hosted::accounts::enabled()
        && (linked_amberite_user_id(state).await? != publishing_account
            || shared_attachment(instance_id, state)
                .await?
                .is_none_or(|current| current.id != attachment.id))
    {
        return Err(crate::ErrorKind::InputError(
            "The account or shared instance changed while publishing.".into(),
        )
        .into());
    }

    match result {
        Ok(version) => {
            tracing::info!(
                instance_id,
                shared_instance_id = %attachment.id,
                version,
                "Published shared instance content"
            );
            crate::state::set_shared_instance_sync_status(
                instance_id,
                ContentSetSyncStatus::UpToDate,
                Some(version),
                Some(version),
                &state.pool,
            )
            .await?;
            Ok(())
        }
        Err(error) => {
            tracing::warn!(
                instance_id,
                shared_instance_id = %attachment.id,
                error = %error,
                "Failed to publish shared instance content"
            );
            crate::state::set_shared_instance_sync_status(
                instance_id,
                ContentSetSyncStatus::Error,
                attachment.applied_version,
                attachment.latest_version,
                &state.pool,
            )
            .await?;
            Err(error)
        }
    }
}

pub(super) async fn publish_current_content(
    instance_id: &str,
    shared_instance_id: &str,
    file_paths: &[String],
    server_core: Option<&str>,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
    report: &mut PublishReport,
    state: &State,
) -> crate::Result<i32> {
    let _store_lease = state.content_store.lease().await;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    report.instance_name = Some(metadata.instance.name.clone());
    report.game_version =
        Some(metadata.applied_content_set.game_version.clone());
    report.loader =
        Some(metadata.applied_content_set.loader.as_str().to_string());
    report.loader_version = metadata.applied_content_set.loader_version.clone();
    ensure_shareable_link(&metadata.link)?;
    update_remote_instance(
        shared_instance_id,
        shared_instance_name(metadata.instance.name.clone()),
        state,
    )
    .await?;
    report.step("Updated shared instance details");
    let modpack_id = shared_modpack_id(&metadata.link);
    report.modpack_id = modpack_id.clone();
    let snapshot = collect_publish_snapshot(&metadata, state).await?;
    report.step("Collected instance content");
    let shared_files = if CONFIG_SYNC_ENABLED {
        shared_file_candidates(
            &metadata.instance.path,
            &snapshot.shared_files,
            file_paths,
            state,
        )
        .await?
    } else {
        Vec::new()
    };
    let modrinth_ids = snapshot.version_ids;
    let mut external_files = snapshot.external_files;
    external_files.extend(shared_files);
    tracing::debug!(
        instance_id,
        shared_instance_id,
        modpack_id = modpack_id.as_deref().unwrap_or("none"),
        modrinth_id_count = modrinth_ids.len(),
        external_file_count = external_files.len(),
        "Creating shared instance version"
    );
    report.modrinth_ids = modrinth_ids.clone();
    let hashed_files =
        hash_external_files(&metadata.instance.path, &external_files, state)
            .await?;
    report.files = hashed_files
        .iter()
        .map(|file| PublishReportFile {
            file_name: file.file_name.clone(),
            file_type: file.file_type.clone(),
            size: file.size,
            sha512: file.sha512.clone(),
            executable: file.executable,
        })
        .collect();
    report.step(format!("Hashed {} files", hashed_files.len()));
    if let Some(file) = hashed_files.iter().find(|file| file.executable) {
        bail!(crate::ErrorKind::SharedInstanceFileNotAllowed(
            file.file_name.clone()
        ));
    }
    let total_size = hashed_files
        .iter()
        .fold(0_u64, |total, file| total.saturating_add(file.size));
    if total_size > MAX_SHARED_FILES_TOTAL_SIZE {
        bail!(crate::ErrorKind::PoolTooLarge(format!(
            "A shared instance can hold at most {} of files.",
            crate::util::io::format_bytes(MAX_SHARED_FILES_TOTAL_SIZE)
        )));
    }
    let mut version_request = json!({
        "modrinth_ids": modrinth_ids,
        "modpack_id": modpack_id,
        "removed_files": snapshot.removed_files,
        "game_version": metadata.applied_content_set.game_version.clone(),
        "loader": metadata.applied_content_set.loader.as_str(),
        "loader_version": metadata
            .applied_content_set
            .loader_version
            .clone()
            .unwrap_or_default(),
    });
    if let Some(server_core) = server_core {
        version_request["server_core"] = json!(server_core);
    }

    // The encrypted files wait in a folder of their own until the Core has them.
    let folder = state
        .directories
        .caches_dir()
        .join("pool")
        .join(uuid::Uuid::new_v4().to_string());
    let result = push_version(
        instance_id,
        &metadata,
        shared_instance_id,
        version_request,
        &hashed_files,
        &folder,
        on_progress,
        report,
        state,
    )
    .await;
    if let Err(error) = tokio::fs::remove_dir_all(&folder).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            folder = %folder.display(),
            error = %error,
            "Could not remove a push's encrypted files"
        );
    }
    result
}

/// Encrypts the files, creates the version, stores the files on the Core the backend picks, and
/// waits until the Core has confirmed them.
#[allow(clippy::too_many_arguments)]
async fn push_version(
    instance_id: &str,
    metadata: &crate::state::InstanceMetadata,
    shared_instance_id: &str,
    mut version_request: serde_json::Value,
    hashed_files: &[HashedExternalFile],
    folder: &std::path::Path,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
    report: &mut PublishReport,
    state: &State,
) -> crate::Result<i32> {
    let previous = previous_versions(shared_instance_id, state).await?;
    let carried = carried_config_files(&previous, hashed_files);
    let file_count = hashed_files.len() + carried.len();
    if file_count > MAX_SHARED_FILES {
        bail!(crate::ErrorKind::PoolTooLarge(format!(
            "A shared instance can hold at most {MAX_SHARED_FILES} files, and this push has {file_count}."
        )));
    }
    let keys = file_keys(&previous, hashed_files);
    crate::util::io::create_dir_all(folder).await?;
    // Collected first so the stream holds plain futures, not a borrowing closure, which keeps
    // the push future Send for Tauri commands.
    let encryptions = hashed_files
        .iter()
        .zip(&keys)
        .map(|(file, key)| pool::encrypt_file(key, file.content.path(), folder))
        .collect::<Vec<_>>();
    let mut objects = futures::stream::iter(encryptions)
        .buffered(UPLOAD_CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?;
    // A file written to between hashing and encrypting would reach the Core with a P that its
    // bytes no longer match, and every member's install would fail on it.
    if let Some(file) =
        hashed_files
            .iter()
            .zip(&objects)
            .find_map(|(file, object)| {
                (object.plain != file.sha256).then_some(file)
            })
    {
        bail!(crate::ErrorKind::InputError(format!(
            "{} changed while it was being pushed. Push again.",
            file.file_name
        )));
    }
    report.step(format!("Encrypted {} files", objects.len()));

    let mut external_files = hashed_files
        .iter()
        .zip(&keys)
        .zip(&objects)
        .map(|((file, key), object)| ExternalFileData {
            file_name: file.file_name.clone(),
            file_type: file.file_type.clone(),
            file_size: file.size,
            sha1: file.sha1.clone(),
            sha256: hex::encode(file.sha256),
            object: hex::encode(object.object),
            object_size: object.size,
            key: (!crypto::is_content_type(&file.file_type))
                .then(|| hex::encode(key)),
        })
        .collect::<Vec<_>>();
    for (file, object) in carried {
        external_files.push(file);
        objects.push(object);
    }
    let root = hex::encode(crypto::root(objects.iter().map(|o| o.object)));
    version_request["external_files"] = json!(external_files);
    version_request["root"] = json!(root);

    let response =
        create_remote_version(shared_instance_id, version_request, state)
            .await?;
    let response = match response {
        SharedInstanceRemoteResponse::Available(response) => response,
        SharedInstanceRemoteResponse::Unavailable(reason) => {
            if let Some(attachment) = metadata.shared_instance.as_ref() {
                handle_unavailable_shared_instance_if_current_user(
                    instance_id,
                    attachment,
                    reason,
                    state,
                )
                .await?;
            }

            return Err(shared_instance_unavailable_error(reason));
        }
    };
    report.created_version = Some(response.version);
    report.step(format!("Created version {}", response.version));
    if response.ready {
        return Ok(response.version);
    }

    if let Some(store) = &response.store {
        let sent = pool::store_version(
            &store.core,
            &store.ticket,
            &root,
            &objects,
            on_progress,
        )
        .await?;
        report.requested_files = Some(
            external_files
                .iter()
                .filter(|file| sent.contains(&file.object))
                .map(|file| file.file_name.clone())
                .collect(),
        );
        report.step(format!("Stored {} files on the Core", sent.len()));
    }

    for _ in 0..CONFIRM_ATTEMPTS {
        tokio::time::sleep(CONFIRM_INTERVAL).await;
        if get_remote_version(shared_instance_id, response.version, state)
            .await?
            .is_some_and(|version| version.ready)
        {
            report.step("The Core confirmed the files");
            return Ok(response.version);
        }
    }
    Err(crate::ErrorKind::PoolPushUnconfirmed.into())
}

/// The versions a push builds on: the latest one, then the version a crashed push left
/// unconfirmed one past it, if there is one.
async fn previous_versions(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<Vec<InstanceVersionResponse>> {
    let mut versions = Vec::new();
    if let SharedInstanceRemoteResponse::Available(latest) =
        get_latest_remote_version_optional_unavailable(
            shared_instance_id,
            state,
        )
        .await?
    {
        let pending = if latest.ready {
            get_remote_version(shared_instance_id, latest.version + 1, state)
                .await?
        } else {
            None
        };
        versions.push(latest);
        versions.extend(pending);
    }
    Ok(versions)
}

/// The config files of the latest ready version that this push does not send again. Configs
/// are picked per push, so each version carries the earlier ones forward and lists every config
/// shared so far: any version stands alone, however many older ones the backend has dropped.
/// The Core still holds their objects, since it keeps the latest ready version.
fn carried_config_files(
    previous: &[InstanceVersionResponse],
    pushed: &[HashedExternalFile],
) -> Vec<(ExternalFileData, pool::PoolObject)> {
    let Some(latest) = previous.first().filter(|version| version.ready) else {
        return Vec::new();
    };
    let pushed = pushed
        .iter()
        .map(|file| (file.file_type.as_str(), file.file_name.as_str()))
        .collect::<HashSet<_>>();
    latest
        .external_files
        .iter()
        .filter(|file| {
            matches!(
                file.file_type.as_str(),
                CONFIG_FILE_TYPE | CONFIG_BUNDLE_FILE_TYPE
            ) && !pushed
                .contains(&(file.file_type.as_str(), file.file_name.as_str()))
        })
        .filter_map(|file| {
            let carried = carried_config_file(file);
            if carried.is_none() {
                tracing::warn!(
                    file_name = %file.file_name,
                    "A shared config file has an incomplete manifest entry and is not carried forward"
                );
            }
            carried
        })
        .collect()
}

fn carried_config_file(
    file: &ExternalFileResponse,
) -> Option<(ExternalFileData, pool::PoolObject)> {
    let object = file.object.as_deref().and_then(pool::parse_hash)?;
    let object_size = file.object_size?;
    let sha256 = file.sha256.as_deref().and_then(pool::parse_hash)?;
    Some((
        ExternalFileData {
            file_name: file.file_name.clone(),
            file_type: file.file_type.clone(),
            file_size: file
                .file_size
                .and_then(|size| u64::try_from(size).ok())?,
            sha1: file.sha1.clone()?,
            sha256: hex::encode(sha256),
            object: hex::encode(object),
            object_size,
            key: Some(file.key.clone()?),
        },
        pool::PoolObject {
            object,
            size: object_size,
            path: None,
            plain: sha256,
        },
    ))
}

/// Each file's key. Content files derive theirs from P. Other files keep the key an earlier
/// version gave the same P, so an unchanged file stays the same object, and get a new random
/// key otherwise. A push that crashed after creating its version left it unconfirmed one past
/// the latest ready one; its keys win, so pushing the same files again gives the same root and
/// resumes that upload instead of waiting for it to expire.
fn file_keys(
    previous: &[InstanceVersionResponse],
    files: &[HashedExternalFile],
) -> Vec<[u8; 32]> {
    let mut keys = HashMap::new();
    for file in previous
        .iter()
        .rev()
        .flat_map(|version| &version.external_files)
    {
        if let (Some(p), Some(key)) = (
            file.sha256.as_deref().and_then(pool::parse_hash),
            file.key.as_deref().and_then(pool::parse_hash),
        ) {
            keys.entry(p).or_insert(key);
        }
    }

    files
        .iter()
        .map(|file| {
            if crypto::is_content_type(&file.file_type) {
                return crypto::content_key(&file.sha256);
            }
            *keys.entry(file.sha256).or_insert_with(|| {
                let mut key = [0; 32];
                rand::fill(&mut key);
                key
            })
        })
        .collect()
}

pub(super) async fn collect_shareable_files(
    instance_path: &std::path::Path,
) -> crate::Result<Vec<SharedInstancePublishFile>> {
    let mut files = Vec::new();
    let root = instance_path.to_path_buf();
    let mut walker = WalkDir::new(instance_path).filter(move |entry| {
        let root = root.clone();
        async move {
            let is_dir = entry
                .file_type()
                .await
                .is_ok_and(|file_type| file_type.is_dir());
            let entry_path = entry.path();
            let skip_dir = is_dir
                && entry_path.strip_prefix(&root).is_ok_and(|relative| {
                    !is_shareable_directory(
                        &relative.to_string_lossy().replace('\\', "/"),
                    )
                });
            if skip_dir {
                Filtering::IgnoreDir
            } else {
                Filtering::Continue
            }
        }
    });

    while let Some(entry) = walker.next().await {
        let entry = entry.context("failed to read instance directory")?;
        if !entry.file_type().await?.is_file() {
            continue;
        }

        let entry_path = entry.path();
        let relative_path = entry_path.strip_prefix(instance_path)?;
        let path = relative_path.to_string_lossy().replace('\\', "/");
        if !is_shareable_file_path(&path) {
            continue;
        }
        let size = entry.metadata().await?.len();
        let blocked =
            is_executable_content(&read_file_head(&entry_path).await?);
        files.push(SharedInstancePublishFile {
            path,
            size,
            blocked,
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

async fn read_file_head(path: &std::path::Path) -> crate::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;

    let mut head = Vec::with_capacity(FILE_SCAN_HEAD_SIZE);
    tokio::fs::File::open(path)
        .await?
        .take(FILE_SCAN_HEAD_SIZE as u64)
        .read_to_end(&mut head)
        .await?;
    Ok(head)
}

/// Each selected file is uploaded on its own under its instance-relative
/// path, matching how linked servers publish files.
async fn shared_file_candidates(
    instance_path: &str,
    local_files: &[SharedInstancePublishFile],
    selected_paths: &[String],
    state: &State,
) -> crate::Result<Vec<ExternalFileCandidate>> {
    let local_files = local_files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<HashMap<_, _>>();
    let selected_paths = selected_paths
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for path in &selected_paths {
        match local_files.get(path) {
            None => bail!(crate::ErrorKind::InputError(format!(
                "file {path} is unavailable for sharing"
            ))),
            Some(file) if file.blocked => {
                bail!(crate::ErrorKind::SharedInstanceFileNotAllowed(
                    path.to_string()
                ))
            }
            Some(_) => {}
        }
    }
    ensure_shared_file_count(&selected_paths)?;

    let instance_path = state.directories.instances_dir().join(instance_path);
    let mut total_size = 0_u64;
    let mut candidates = Vec::with_capacity(selected_paths.len());
    for path in selected_paths {
        let source = instance_path.join(path);
        let size = crate::util::io::metadata(&source).await?.len();
        total_size = total_size.saturating_add(size);
        if total_size > MAX_SHARED_FILES_TOTAL_SIZE {
            bail!(crate::ErrorKind::PoolTooLarge(format!(
                "A shared instance can hold at most {} of files.",
                crate::util::io::format_bytes(MAX_SHARED_FILES_TOTAL_SIZE)
            )));
        }
        candidates.push(ExternalFileCandidate {
            file_name: path.to_string(),
            file_type: CONFIG_FILE_TYPE.to_string(),
            source: ExternalFileSource::SharedFile(source),
        });
    }

    Ok(candidates)
}

fn ensure_shared_file_count(paths: &BTreeSet<&str>) -> crate::Result<()> {
    if paths.len() <= MAX_SHARED_FILES {
        return Ok(());
    }

    let mut folder_entry_counts = HashMap::new();
    for path in paths {
        if let Some((folder, _)) = path.split_once('/') {
            *folder_entry_counts.entry(folder).or_insert(0_usize) += 1;
        }
    }

    if let Some((folder, count)) = folder_entry_counts
        .into_iter()
        .filter(|(_, count)| *count > MAX_SHARED_FILES)
        .max_by_key(|(_, count)| *count)
    {
        bail!(crate::ErrorKind::InputError(format!(
            "the \"{folder}\" folder has too many files to share ({count}; maximum {MAX_SHARED_FILES}); select fewer files from this folder"
        )));
    }

    bail!(crate::ErrorKind::InputError(format!(
        "too many files were selected to share ({}; maximum {MAX_SHARED_FILES}); select fewer files",
        paths.len()
    )))
}

pub(super) fn shared_modpack_id(link: &InstanceLink) -> Option<String> {
    match link {
        InstanceLink::ModrinthModpack { version_id, .. } => {
            Some(version_id.clone())
        }
        InstanceLink::ServerProjectModpack {
            content_version_id, ..
        } => Some(content_version_id.clone()),
        InstanceLink::SharedInstance {
            modpack_version_id: Some(version_id),
            ..
        } => Some(version_id.clone()),
        _ => None,
    }
}

pub(super) fn ensure_shareable_link(link: &InstanceLink) -> crate::Result<()> {
    if matches!(link, InstanceLink::ImportedModpack { .. }) {
        bail!(crate::ErrorKind::InputError(
            "you must unlink this modpack to share your instance".to_string(),
        ));
    }

    Ok(())
}

/// A publish candidate with its content, size, SHA-512 (for the publish report), SHA-1 (which a
/// member's install compares) and SHA-256, which is the pool's P.
pub(super) struct HashedExternalFile {
    file_name: String,
    file_type: String,
    content: crate::state::content_store::ReadableContent,
    size: u64,
    sha512: String,
    sha1: String,
    sha256: [u8; 32],
    executable: bool,
}

async fn hash_external_files(
    instance_path: &str,
    candidates: &[ExternalFileCandidate],
    state: &State,
) -> crate::Result<Vec<HashedExternalFile>> {
    let hashes = candidates
        .iter()
        .map(|candidate| hash_external_file(instance_path, candidate, state))
        .collect::<Vec<_>>();
    futures::stream::iter(hashes)
        .buffered(UPLOAD_CONCURRENCY)
        .try_collect()
        .await
}

async fn hash_external_file(
    instance_path: &str,
    candidate: &ExternalFileCandidate,
    state: &State,
) -> crate::Result<HashedExternalFile> {
    use tokio::io::AsyncReadExt;

    let content = external_file_source(instance_path, candidate, state).await?;
    let mut file = tokio::fs::File::open(content.path()).await?;
    let mut hasher = sha2::Sha512::new();
    let mut sha1 = sha1_smol::Sha1::new();
    let mut sha256 = sha2::Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut head = Vec::with_capacity(FILE_SCAN_HEAD_SIZE);
    let mut size = 0_u64;
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        sha1.update(&buffer[..read]);
        sha256.update(&buffer[..read]);
        let remaining = FILE_SCAN_HEAD_SIZE - head.len();
        head.extend_from_slice(&buffer[..read.min(remaining)]);
        size += read as u64;
    }

    Ok(HashedExternalFile {
        file_name: candidate.file_name.clone(),
        file_type: candidate.file_type.clone(),
        content,
        size,
        sha512: crate::util::content_hash::encode_hex(
            hasher.finalize().as_slice(),
        ),
        sha1: sha1.digest().to_string(),
        sha256: sha256.finalize().into(),
        executable: is_executable_content(&head),
    })
}

async fn external_file_source(
    instance_path: &str,
    candidate: &ExternalFileCandidate,
    state: &State,
) -> crate::Result<crate::state::content_store::ReadableContent> {
    Ok(match &candidate.source {
        ExternalFileSource::InstanceFile(file_path) => {
            let instance = crate::state::instances::adapters::sqlite::instance_rows::get_instance_by_path(instance_path, &state.pool).await?
				.ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
            let file = crate::state::instances::adapters::sqlite::content_rows::get_instance_file_by_relative_path(&instance.id, file_path, &state.pool).await?
				.ok_or(crate::ErrorKind::InputError("shared content file is not registered".to_string()))?;
            state.content_store.read_path(&file, instance_path).await?
        }
        ExternalFileSource::SharedFile(path) => {
            crate::state::content_store::ReadableContent::Local(path.clone())
        }
    })
}

pub(super) async fn shared_attachment(
    instance_id: &str,
    state: &State,
) -> crate::Result<Option<SharedInstanceAttachment>> {
    Ok(crate::state::get_instance(instance_id, &state.pool)
        .await?
        .and_then(|metadata| metadata.shared_instance))
}

pub(crate) async fn sync_shared_instance_icon(
    instance_id: &str,
    icon_path: Option<&str>,
    state: &State,
) -> crate::Result<()> {
    let Some(attachment) = shared_attachment(instance_id, state).await? else {
        return Ok(());
    };
    if attachment.role != SharedInstanceRole::Owner {
        return Ok(());
    }

    update_remote_instance_icon(&attachment.id, icon_path, state).await
}

pub(super) async fn shared_instance_for_invites(
    instance_id: &str,
    user_count: usize,
    state: &State,
) -> crate::Result<(crate::state::InstanceMetadata, SharedInstanceAttachment)> {
    tracing::debug!(instance_id, user_count, "Sharing instance for invite");
    let (metadata, attachment) =
        owned_shared_instance(instance_id, state).await?;

    ensure_owner(&attachment)?;
    update_remote_instance_icon(
        &attachment.id,
        metadata.instance.icon_path.as_deref(),
        state,
    )
    .await?;

    Ok((metadata, attachment))
}

/// The instance's share, created and published as its owner's when it has none: before the
/// first invite, or before the instance's first server.
pub(super) async fn owned_shared_instance(
    instance_id: &str,
    state: &State,
) -> crate::Result<(crate::state::InstanceMetadata, SharedInstanceAttachment)> {
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let attachment = match metadata.shared_instance.clone() {
        Some(attachment) => {
            tracing::debug!(
                instance_id,
                shared_instance_id = %attachment.id,
                role = attachment.role.as_str(),
                "Using existing shared instance attachment"
            );
            attachment
        }
        None => {
            ensure_shareable_link(&metadata.link)?;
            tracing::info!(instance_id, "Creating shared instance");
            let remote = create_remote_instance(
                shared_instance_name(metadata.instance.name.clone()),
                state,
            )
            .await?;
            let linked_user_id = linked_amberite_user_id(state).await?;
            tracing::info!(
                instance_id,
                shared_instance_id = %remote.id,
                "Created remote shared instance"
            );
            crate::state::attach_shared_instance(
                instance_id,
                crate::state::SharedInstanceAttachmentInput {
                    id: remote.id.clone(),
                    role: SharedInstanceRole::Owner,
                    manager_id: None,
                    server_manager_name: None,
                    server_manager_icon_url: None,
                    linked_user_id,
                    status: ContentSetSyncStatus::Unknown,
                    applied_version: None,
                    applied_backtrack_version: None,
                    latest_version: None,
                },
                &state.pool,
            )
            .await?;
            tracing::debug!(
                instance_id,
                shared_instance_id = %remote.id,
                "Attached local instance as shared instance owner"
            );
            publish_shared_instance_inner(
                instance_id,
                &[],
                None,
                &|_, _| {},
                state,
            )
            .await?;
            shared_attachment(instance_id, state).await?.ok_or(
                crate::ErrorKind::InputError(
                    "shared instance attachment was not persisted".to_string(),
                ),
            )?
        }
    };

    Ok((metadata, attachment))
}

pub(super) fn ensure_owner(
    attachment: &SharedInstanceAttachment,
) -> crate::Result<()> {
    if attachment.role == SharedInstanceRole::Owner {
        return Ok(());
    }

    bail!(crate::ErrorKind::InputError(
        "only the owner instance can manage shared instance users".to_string(),
    ))
}

pub(super) fn ensure_member(
    attachment: &SharedInstanceAttachment,
) -> crate::Result<()> {
    if attachment.role.is_member() {
        return Ok(());
    }

    bail!(crate::ErrorKind::InputError(
        "only shared instance members can unlink from shared instances"
            .to_string(),
    ))
}

pub(super) fn file_type(project_type: ProjectType) -> String {
    project_type.get_name().to_string()
}
