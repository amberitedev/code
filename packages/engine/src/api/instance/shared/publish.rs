use super::client::*;
use super::diff::*;
use super::install::*;
use super::report::{PublishReport, PublishReportFile};
use super::types::*;
use super::*;
use crate::self_hosted::pool;
use futures::{StreamExt, TryStreamExt};
use sharing_client::push;

#[tracing::instrument(skip(state))]
pub async fn unpublish_shared_instance(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    let _shared_instance_lock =
        state.instance_locks.lock_shared(instance_id).await;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let Some(attachment) = metadata.shared_instance.clone() else {
        return Ok(());
    };
    ensure_owner(&attachment)?;

    delete_remote_instance(&attachment.id, state).await?;
    super::history::forget_share_history(&attachment.id, state).await?;
    detach_local_shared_instance(instance_id, metadata.link, state).await?;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn unpublish_remote_shared_instance(
    state: &Engine,
    shared_instance_id: &str,
) -> crate::Result<()> {
    delete_remote_instance(shared_instance_id, state).await
}

#[tracing::instrument(skip(state))]
pub async fn leave_remote_shared_instance(
    state: &Engine,
    shared_instance_id: &str,
) -> crate::Result<()> {
    let Some(user_id) = linked_amberite_user_id(state).await? else {
        bail!(crate::ErrorKind::InputError(
            "you must be signed in to leave a shared instance".to_string(),
        ));
    };

    remove_remote_users(shared_instance_id, vec![user_id], state).await
}

#[tracing::instrument(skip(state))]
pub async fn unlink_shared_instance(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    let _shared_instance_lock =
        state.instance_locks.lock_shared(instance_id).await;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let Some(attachment) = metadata.shared_instance.clone() else {
        return Ok(());
    };
    ensure_member(&attachment)?;

    detach_local_shared_instance(instance_id, metadata.link, state).await?;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(())
}

pub(super) async fn detach_local_shared_instance(
    instance_id: &str,
    link: InstanceLink,
    state: &Engine,
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

/// `file_paths` are the files to send with this version; files the latest version
/// shares stay shared unless they are in `removed_paths` or gone from the instance.
/// `on_progress` receives the uploaded and total bytes of the files the
/// shared instances service asks for. `server_core` is a Core the owner owns
/// that runs a linked server of this instance; the backend keeps the share's
/// home copy there.
#[tracing::instrument(skip(state, on_progress))]
pub async fn publish_shared_instance(
    state: &Engine,
    instance_id: &str,
    file_paths: Vec<String>,
    removed_paths: Vec<String>,
    server_core: Option<String>,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
) -> crate::Result<SharedInstanceAttachment> {
    publish_shared_instance_inner(
        instance_id,
        &file_paths,
        &removed_paths,
        server_core.as_deref(),
        on_progress,
        state,
    )
    .await?;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    shared_attachment(instance_id, state).await?.ok_or(
        crate::ErrorKind::InputError(
            "shared instance attachment was not persisted".to_string(),
        )
        .into(),
    )
}

#[tracing::instrument(skip(state))]
pub async fn get_shared_instance_publish_preview(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<Option<SharedInstancePublishPreview>> {
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or(crate::ErrorKind::InputError("unknown instance".to_string()))?;
    let Some(attachment) = metadata.shared_instance.clone() else {
        return Ok(None);
    };
    ensure_owner(&attachment)?;

    let (version, snapshot) = tokio::try_join!(
        get_latest_remote_version_optional_unavailable(&attachment.id, state,),
        collect_publish_snapshot(&metadata, state),
    )?;
    let version = match version {
        SharedInstanceRemoteResponse::Available(version) => version,
        SharedInstanceRemoteResponse::Unavailable(reason) => {
            handle_unavailable_shared_instance_if_current_user(
                instance_id,
                &attachment,
                reason,
                state,
            )
            .await?;
            return Err(shared_instance_unavailable_error(reason));
        }
    };
    ensure_owner_up_to_date(instance_id, &attachment, version.version, state)
        .await?;
    let diffs =
        shared_instance_publish_diffs(&metadata, &version, &snapshot, state)
            .await?;
    let shared = sharing_client::versions::configs(&version)
        .map(|file| file.file_name.as_str())
        .collect::<HashSet<_>>();
    let mut files = snapshot.shared_files;
    for file in &mut files {
        file.shared = shared.contains(file.path.as_str());
    }
    set_shared_instance_publish_status(
        instance_id,
        &attachment,
        version.version,
        !diffs.is_empty(),
        state,
    )
    .await?;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(Some(SharedInstancePublishPreview {
        shared_instance_id: attachment.id,
        latest_version: version.version,
        diffs,
        files,
    }))
}

pub(super) async fn shared_instance_install_modpack(
    version: &InstanceVersion,
    state: &Engine,
) -> crate::Result<Option<crate::install::SharedInstanceInstallModpack>> {
    let Some(modpack_id) = sharing_client::versions::modpack_id(version) else {
        return Ok(None);
    };
    let modpack_version = CachedEntry::get_version(
        modpack_id,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
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
        &state.sessions,
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

/// The shared content installed now: Modrinth version ids, and the external files with the
/// SHA-1 of each.
pub(super) async fn current_shared_content(
    metadata: &crate::state::InstanceMetadata,
    include_local_content: bool,
    state: &Engine,
) -> crate::Result<(Vec<String>, BTreeMap<ExternalFileKey, Option<String>>)> {
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
        &state.sessions,
    )
    .await?
    .into_iter()
    .map(|file| (file.hash.clone(), file))
    .collect::<HashMap<_, _>>();
    let mut version_ids = Vec::new();
    let mut external_files = BTreeMap::new();
    for entry in entries {
        let include_entry = entry.source_kind.is_shared_instance_managed()
            || (include_local_content
                && entry.source_kind == ContentSourceKind::Local);
        if !include_entry || !entry.enabled {
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
            external_files.insert(
                ExternalFileKey {
                    content_type: entry.project_type.into(),
                    path: enabled_file_name(&file.file_name),
                },
                Some(file.sha1.clone()),
            );
        }
    }
    dedupe_strings(&mut version_ids);
    Ok((version_ids, external_files))
}

pub(super) struct CurrentPublishSnapshot {
    pub(super) version_ids: Vec<String>,
    pub(super) external_files: Vec<ExternalFileCandidate>,
    pub(super) disabled_project_ids: HashSet<String>,
    pub(super) disabled_version_ids: Vec<String>,
    pub(super) disabled_external_files: BTreeSet<ExternalFileKey>,
    pub(super) shared_files: Vec<SharedInstancePublishFile>,
    pub(super) removed_files: Vec<RemovedFile>,
    pub(super) effective_version_ids: Vec<String>,
}

/// Cached metadata can outlive a deleted or hidden version. Only publish version
/// IDs that other members can resolve; retain installed files as uploads.
pub(super) async fn collect_publish_snapshot(
    metadata: &crate::state::InstanceMetadata,
    state: &Engine,
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
    // The modpack's files the owner removed or turned off.
    let removed_files =
        push::removed_files(modpack_files.iter().filter(|file| {
            match &file.version_id {
                Some(version_id) => {
                    !enabled_versions.contains(version_id.as_str())
                }
                None => !enabled_paths.contains(file.path.as_str()),
            }
        }));
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
        &state.sessions,
    )
    .await?
    .into_iter()
    .map(|version| version.id)
    .collect::<HashSet<_>>();
    let mut seen_version_ids = HashSet::new();
    let mut rows = Vec::new();
    let mut disabled_project_ids = HashSet::new();
    let mut disabled_version_ids = Vec::new();
    let mut seen_disabled_version_ids = HashSet::new();
    let mut disabled_external_files = BTreeSet::new();

    for item in items {
        if item.enabled {
            let version_id = item
                .version
                .as_ref()
                .map(|version| version.id.clone())
                .filter(|id| available_version_ids.contains(id));
            if let Some(version_id) = &version_id
                && seen_version_ids.insert(version_id.clone())
            {
                effective_version_ids.push(version_id.clone());
            }
            if version_id.is_none() && item.file_path.is_empty() {
                continue;
            }
            rows.push(push::ContentRow {
                from_modpack: modpack_id.is_some()
                    && matches!(
                        item.source_kind,
                        Some(
                            ContentSourceKind::ModrinthModpack
                                | ContentSourceKind::ServerProject
                        )
                    ),
                path: item.file_path,
                file_name: item.file_name,
                file_type: file_type(item.project_type),
                version_id,
            });
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

    let read = push::read_content(rows, &modpack_files);
    let mut seen_external_files = HashSet::new();
    let external_files = read
        .carried
        .into_iter()
        .filter(|row| {
            seen_external_files
                .insert((row.path.clone(), row.file_type.clone()))
        })
        .map(|row| ExternalFileCandidate {
            file_name: row.file_name,
            file_type: row.file_type,
            source: ExternalFileSource::InstanceFile(row.path),
        })
        .collect();
    let mut version_ids = read.modrinth_ids;
    dedupe_strings(&mut version_ids);

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

pub(super) async fn shared_project_names(
    project_ids: &HashSet<String>,
    state: &Engine,
) -> crate::Result<HashMap<String, String>> {
    let project_id_refs =
        project_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let projects = CachedEntry::get_project_many(
        &project_id_refs,
        Some(CacheBehaviour::Bypass),
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
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
    state: &Engine,
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
    removed_paths: &[String],
    server_core: Option<&str>,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
    state: &Engine,
) -> crate::Result<()> {
    let mut report = PublishReport::new(instance_id, file_paths);
    let result = publish_shared_instance_reported(
        instance_id,
        file_paths,
        removed_paths,
        server_core,
        on_progress,
        &mut report,
        state,
    )
    .await;
    report.finish(&result, &state.publish_reports);
    result
}

async fn publish_shared_instance_reported(
    instance_id: &str,
    file_paths: &[String],
    removed_paths: &[String],
    server_core: Option<&str>,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
    report: &mut PublishReport,
    state: &Engine,
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
    if attachment.applied_version.is_some()
        && let SharedInstanceRemoteResponse::Available(version) =
            get_latest_remote_version_optional_unavailable(
                &attachment.id,
                state,
            )
            .await?
    {
        ensure_owner_up_to_date(
            instance_id,
            &attachment,
            version.version,
            state,
        )
        .await?;
    }
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
        removed_paths,
        server_core,
        on_progress,
        report,
        state,
    )
    .await;

    if crate::self_hosted::accounts::enabled(&state.sessions)
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
            if attachment.restored_from_remote {
                crate::state::set_shared_instance_restored_from_remote(
                    instance_id,
                    false,
                    &state.pool,
                )
                .await?;
            }
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
    removed_paths: &[String],
    server_core: Option<&str>,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
    report: &mut PublishReport,
    state: &Engine,
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
    let mut external_files = snapshot.external_files;
    external_files.extend(shared_files);
    tracing::debug!(
        instance_id,
        shared_instance_id,
        modpack_id = modpack_id.as_deref().unwrap_or("none"),
        modrinth_id_count = snapshot.version_ids.len(),
        external_file_count = external_files.len(),
        "Creating shared instance version"
    );
    report.modrinth_ids = snapshot.version_ids.clone();
    // Held until the push ends, so each path keeps pointing at the file's bytes.
    let sources =
        external_file_sources(&metadata.instance.path, &external_files, state)
            .await?;
    let files = external_files
        .into_iter()
        .zip(&sources)
        .map(|(candidate, source)| push::PushFile {
            file_name: candidate.file_name,
            file_type: candidate.file_type,
            path: source.path().to_path_buf(),
        })
        .collect::<Vec<_>>();
    let (content, files) = push::build_version(
        INSTANCE_SCOPE,
        push::VersionInput {
            modrinth_ids: snapshot.version_ids,
            modpack_id,
            removed_files: snapshot.removed_files,
            game_version: metadata.applied_content_set.game_version.clone(),
            loader: metadata.applied_content_set.loader.as_str().to_owned(),
            loader_version: metadata.applied_content_set.loader_version.clone(),
            files,
        },
    )
    .map_err(|error| crate::ErrorKind::InputError(error.to_string()))?;

    let locate = locate_config(
        state
            .directories
            .instances_dir()
            .join(&metadata.instance.path),
    );
    let result = push::push(
        &push_backend(&state.sessions)?.remote(),
        &pool::STORE,
        shared_instance_id,
        &content,
        server_core,
        files,
        push::KeptConfigs {
            scope: INSTANCE_SCOPE,
            removed: removed_paths,
            locate: &locate,
        },
        &state.directories.caches_dir().join("pool"),
        &mut |event| report_push_event(report, on_progress, event),
    )
    .await;
    match result {
        Ok(pushed) => {
            super::history::record_push(
                shared_instance_id,
                &content,
                &pushed,
                state,
            )
            .await;
            Ok(pushed.version)
        }
        Err(push::PushError::Unavailable(reason)) => {
            let reason = unavailable_reason(reason);
            if reason == SharedInstanceUnavailableReason::AccessRevoked
                && !active_amberite_session_is_configured(state).await?
            {
                bail!(crate::ErrorKind::NotLoggedIn);
            }
            if let Some(attachment) = metadata.shared_instance.as_ref() {
                handle_unavailable_shared_instance_if_current_user(
                    instance_id,
                    attachment,
                    reason,
                    state,
                )
                .await?;
            }
            Err(shared_instance_unavailable_error(reason))
        }
        Err(error) => Err(push_error(error)),
    }
}

/// Where the push finds a config the latest version shares: the instance's copy, while it has
/// one.
fn locate_config(
    instance_path: std::path::PathBuf,
) -> impl Fn(
    String,
) -> futures::future::BoxFuture<
    'static,
    std::io::Result<Option<std::path::PathBuf>>,
> + Send
+ Sync
+ use<> {
    move |file_name: String| {
        let path = instance_path.join(file_name);
        Box::pin(async move {
            Ok(tokio::fs::symlink_metadata(&path)
                .await
                .is_ok_and(|metadata| metadata.is_file())
                .then_some(path))
        })
    }
}

fn report_push_event(
    report: &mut PublishReport,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
    event: push::PushEvent<'_>,
) {
    use push::PushEvent;

    match event {
        PushEvent::Dropped(names) => {
            report.step(format!(
                "Stopped sharing {} config files: {}",
                names.len(),
                names.join(", ")
            ));
        }
        PushEvent::Hashed(files) => {
            report.files = files
                .iter()
                .map(|file| PublishReportFile {
                    file_name: file.file_name.clone(),
                    file_type: file.file_type.clone(),
                    size: file.size,
                    sha256: hex::encode(file.sha256),
                    executable: file.executable,
                })
                .collect();
            report.step(format!("Hashed {} files", files.len()));
        }
        PushEvent::Encrypted(count) => {
            report.step(format!("Encrypted {count} files"));
        }
        PushEvent::Created(version) => {
            report.created_version = Some(version);
            report.step(format!("Created version {version}"));
        }
        PushEvent::Stored { objects, files } => {
            report.requested_files = Some(files);
            report.step(format!("Stored {objects} files on the Core"));
        }
        PushEvent::Progress(uploaded, total) => on_progress(uploaded, total),
        PushEvent::Confirmed => report.step("The Core confirmed the files"),
    }
}

/// The App's error for each way a push fails, worded for the push UI.
fn push_error(error: push::PushError) -> crate::Error {
    use push::PushError;
    use sharing_client::pool::StoreError;

    match error {
        PushError::Executable(file_name) => {
            crate::ErrorKind::SharedInstanceFileNotAllowed(file_name).into()
        }
        PushError::TooLarge => crate::ErrorKind::PoolTooLarge(format!(
            "A shared instance can hold at most {} of files.",
            crate::util::io::format_bytes(contracts::limits::MAX_SHARE_BYTES)
        ))
        .into(),
        PushError::FileChanged(file_name) => {
            crate::ErrorKind::InputError(format!(
                "{file_name} changed while it was being pushed. Push again."
            ))
            .into()
        }
        PushError::NoStorage => crate::ErrorKind::PoolNoStorage.into(),
        PushError::PushInFlight => crate::ErrorKind::PoolPushInFlight.into(),
        PushError::Refused(description) => {
            crate::ErrorKind::PoolTooLarge(description).into()
        }
        PushError::Unconfirmed => crate::ErrorKind::PoolPushUnconfirmed.into(),
        PushError::Store(StoreError::DiskFull) => {
            crate::ErrorKind::PoolDiskFull.into()
        }
        PushError::Store(StoreError::EgressSpent) => {
            crate::ErrorKind::PoolEgressSpent.into()
        }
        PushError::Io(error) => error.into(),
        PushError::Backend(error) => backend_error(error),
        error => anyhow::Error::new(error).into(),
    }
}

/// The files the owner can pick for a push (`rules::list_shareable`), none marked shared yet.
pub(super) async fn collect_shareable_files(
    instance_path: &std::path::Path,
) -> crate::Result<Vec<SharedInstancePublishFile>> {
    let instance_path = instance_path.to_path_buf();
    let listed = tokio::task::spawn_blocking(move || {
        sharing_client::rules::list_shareable(
            INSTANCE_SCOPE,
            &sharing_client::rules::DiskFolder(&instance_path),
            &Default::default(),
            usize::MAX,
        )
    })
    .await
    .context("failed to read instance directory")??;
    Ok(listed
        .files
        .into_iter()
        .map(|file| SharedInstancePublishFile {
            path: file.path,
            size: file.size,
            blocked: file.blocked,
            shared: file.shared,
        })
        .collect())
}

/// Each selected file is uploaded on its own under its instance-relative
/// path, matching how linked servers publish files.
async fn shared_file_candidates(
    instance_path: &str,
    local_files: &[SharedInstancePublishFile],
    selected_paths: &[String],
    state: &Engine,
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

    let instance_path = state.directories.instances_dir().join(instance_path);
    let mut candidates = Vec::with_capacity(selected_paths.len());
    for path in selected_paths {
        let source = instance_path.join(path);
        candidates.push(ExternalFileCandidate {
            file_name: path.to_string(),
            file_type: CONFIG_FILE_TYPE.to_string(),
            source: ExternalFileSource::SharedFile(source),
        });
    }

    Ok(candidates)
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

/// Where each candidate's bytes are, held for as long as the caller reads them.
async fn external_file_sources(
    instance_path: &str,
    candidates: &[ExternalFileCandidate],
    state: &Engine,
) -> crate::Result<Vec<crate::state::content_store::ReadableContent>> {
    let sources = candidates
        .iter()
        .map(|candidate| external_file_source(instance_path, candidate, state))
        .collect::<Vec<_>>();
    futures::stream::iter(sources)
        .buffered(4)
        .try_collect()
        .await
}

async fn external_file_source(
    instance_path: &str,
    candidate: &ExternalFileCandidate,
    state: &Engine,
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
    state: &Engine,
) -> crate::Result<Option<SharedInstanceAttachment>> {
    Ok(crate::state::get_instance(instance_id, &state.pool)
        .await?
        .and_then(|metadata| metadata.shared_instance))
}

pub(crate) async fn sync_shared_instance_icon(
    instance_id: &str,
    icon_path: Option<&str>,
    icon_config: Option<&IconConfig>,
    state: &Engine,
) -> crate::Result<()> {
    let Some(attachment) = shared_attachment(instance_id, state).await? else {
        return Ok(());
    };
    if attachment.role != SharedInstanceRole::Owner {
        return Ok(());
    }

    update_remote_instance_icon(&attachment.id, icon_path, icon_config, state)
        .await
}

pub(super) async fn shared_instance_for_invites(
    instance_id: &str,
    user_count: usize,
    state: &Engine,
) -> crate::Result<(crate::state::InstanceMetadata, SharedInstanceAttachment)> {
    tracing::debug!(instance_id, user_count, "Sharing instance for invite");
    let (metadata, attachment) =
        owned_shared_instance(instance_id, state).await?;

    ensure_owner(&attachment)?;
    update_remote_instance_icon(
        &attachment.id,
        metadata.instance.icon_path.as_deref(),
        metadata.icon_config.as_ref(),
        state,
    )
    .await?;

    Ok((metadata, attachment))
}

/// The instance's share, created and published as its owner's when it has none: before the
/// first invite, or before the instance's first server.
pub(super) async fn owned_shared_instance(
    instance_id: &str,
    state: &Engine,
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
            let remote_id = create_remote_instance(
                shared_instance_name(metadata.instance.name.clone()),
                state,
            )
            .await?;
            let linked_user_id = linked_amberite_user_id(state).await?;
            tracing::info!(
                instance_id,
                shared_instance_id = %remote_id,
                "Created remote shared instance"
            );
            crate::state::attach_shared_instance(
                instance_id,
                crate::state::SharedInstanceAttachmentInput {
                    id: remote_id.clone(),
                    role: SharedInstanceRole::Owner,
                    restored_from_remote: false,
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
                shared_instance_id = %remote_id,
                "Attached local instance as shared instance owner"
            );
            publish_shared_instance_inner(
                instance_id,
                &[],
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

async fn ensure_owner_up_to_date(
    instance_id: &str,
    attachment: &SharedInstanceAttachment,
    latest_version: i32,
    state: &Engine,
) -> crate::Result<()> {
    if attachment
        .applied_version
        .is_none_or(|applied| applied >= latest_version)
    {
        return Ok(());
    }

    let status = match attachment.status {
        ContentSetSyncStatus::Stale => ContentSetSyncStatus::Stale,
        _ => ContentSetSyncStatus::UpdateAvailable,
    };
    crate::state::set_shared_instance_sync_status(
        instance_id,
        status,
        attachment.applied_version,
        Some(latest_version),
        &state.pool,
    )
    .await?;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;
    bail!(crate::ErrorKind::SharedInstanceOwnerBehind)
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
