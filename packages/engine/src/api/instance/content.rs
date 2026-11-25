use std::sync::Arc;

use contracts::Side;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::api::pack::sandboxed_install::{
    instance_dir, instance_metadata, open_folder, pack_kind, place_turned_on,
    repair_server_pack,
};
use crate::state::instances::adapters::filesystem::server_addon_type;
use crate::state::instances::adapters::sqlite::content_rows::{
    self, EntrySides, SkippedFile, TrackedFile,
};
use crate::state::{
    CacheBehaviour, ContentFile, ContentItem, ContentRequirement, ContentSet,
    ContentSourceKind, Dependency, Engine, InstanceInstallCandidate,
    InstanceInstallTarget, InstanceLink, InstanceMetadata, LinkedModpackInfo,
    ProjectType,
};
use crate::util::sandbox::{self, Folder};

#[tracing::instrument(skip(state))]
pub async fn sync_content_files(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<Vec<crate::state::instances::InstanceFile>> {
    crate::state::sync_content_files(instance_id, state).await
}

#[tracing::instrument(skip(state))]
pub async fn list_content_sets(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<Vec<ContentSet>> {
    crate::state::list_content_sets(instance_id, &state.pool).await
}

#[tracing::instrument(skip(state))]
pub async fn get_projects(
    state: &Engine,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<DashMap<String, ContentFile>> {
    crate::state::get_content_projects(
        instance_id,
        None,
        cache_behaviour,
        state,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_installed_project_ids(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<Vec<String>> {
    crate::state::get_installed_project_ids_for_instance(
        instance_id,
        None,
        state,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_install_candidates(
    state: &Engine,
    project_id: &str,
    project_type: ProjectType,
    targets: Vec<InstanceInstallTarget>,
) -> crate::Result<Vec<InstanceInstallCandidate>> {
    crate::state::get_instance_install_candidates(
        project_id,
        project_type,
        &targets,
        &state.pool,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_content_items(
    state: &Engine,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    let mut items =
        crate::state::list_indexed_content(instance_id, cache_behaviour, state)
            .await?;
    state
        .hooks()
        .decorate_content(state, instance_id, &mut items)
        .await?;
    Ok(items)
}

#[tracing::instrument(skip(state))]
pub async fn refresh_content_updates(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    crate::state::refresh_content_updates(instance_id, state).await
}

#[tracing::instrument(skip(state))]
pub async fn get_linked_modpack_content(
    state: &Engine,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    crate::state::list_linked_modpack_content(
        instance_id,
        None,
        cache_behaviour,
        state,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_dependencies_as_content_items(
    state: &Engine,
    dependencies: Vec<Dependency>,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentItem>> {
    crate::state::dependencies_to_content_items(
        &dependencies,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await
}

#[tracing::instrument(skip(state))]
pub async fn get_linked_modpack_info(
    state: &Engine,
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Option<LinkedModpackInfo>> {
    crate::state::get_linked_modpack_info(
        instance_id,
        None,
        cache_behaviour,
        state,
    )
    .await
}

// A server's addons can be turned off for one side, and the pack it follows can hold client-only
// files someone turns on for it. Only a server engine answers these.

/// A side an addon can be turned off for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ContentSide {
    Server,
    Player,
}

fn server_only(state: &Engine) -> crate::Result<()> {
    if state.side != Side::Server {
        return Err(refused("only a server engine has this"));
    }
    Ok(())
}

fn refused(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}

/// The enabled path of the addon at `path`, which must be directly in a content folder.
fn addon_path(path: &str) -> crate::Result<String> {
    let path = sandbox::clean(path.trim_end_matches(".disabled"))
        .map_err(|error| refused(error.to_string()))?;
    server_addon_type(&path)
        .map(|_| path.clone())
        .ok_or_else(|| refused(format!("{path} is not an addon")))
}

/// Where an addon is on disk: its enabled path, and the path of the file there now.
fn locate(folder: &Folder, path: &str) -> crate::Result<(String, String)> {
    let disabled = format!("{path}.disabled");
    if folder.exists(path) {
        Ok((path.to_owned(), path.to_owned()))
    } else if folder.exists(&disabled) {
        Ok((path.to_owned(), disabled))
    } else {
        Err(refused(format!("There is no {path}.")))
    }
}

fn rename_enabled(
    folder: &Folder,
    path: &str,
    on_disk: &str,
    enabled: bool,
) -> crate::Result<()> {
    let wanted = if enabled {
        path.to_owned()
    } else {
        format!("{path}.disabled")
    };
    if on_disk != wanted {
        folder.rename(on_disk, &wanted)?;
    }
    Ok(())
}

/// A server that follows a shared instance takes its sides from the share's owner.
fn server_first(metadata: &InstanceMetadata) -> crate::Result<()> {
    if matches!(metadata.link, InstanceLink::SharedInstance { .. }) {
        return Err(refused(
            "This server follows an instance, which its owner shares.",
        ));
    }
    Ok(())
}

fn changed(state: &Engine, instance_id: &str, path: &str) {
    state.events.send(crate::event::Event::PathsChanged {
        instance_id: instance_id.to_owned(),
        paths: vec![path.to_owned()],
    });
}

/// The sides recorded for the addon at its enabled `path`; on for both when none are.
async fn sides(
    state: &Engine,
    instance_id: &str,
    path: &str,
) -> crate::Result<EntrySides> {
    Ok(content_rows::tracked_file(&state.pool, instance_id, path)
        .await?
        .map(|(_, sides)| sides)
        .unwrap_or_default())
}

/// Record the sides of the addon at its enabled `path`. A file the engine does not track yet is
/// tracked as someone's on the server, unless its sides are the default, which needs no record.
async fn save_sides(
    state: &Engine,
    metadata: &InstanceMetadata,
    folder: &Folder,
    path: &str,
    sides: &EntrySides,
) -> crate::Result<()> {
    let id = metadata.instance.id.as_str();
    if content_rows::set_entry_sides(&state.pool, id, path, sides).await?
        || *sides == EntrySides::default()
    {
        return Ok(());
    }
    let on_disk = if folder.exists(path) {
        path.to_owned()
    } else {
        format!("{path}.disabled")
    };
    let sha1 = folder
        .sha1(&on_disk)?
        .ok_or_else(|| refused(format!("There is no {path}.")))?;
    content_rows::upsert_tracked_file(
        &state.pool,
        id,
        &metadata.applied_content_set.id,
        &TrackedFile {
            relative_path: path.to_owned(),
            source_kind: ContentSourceKind::Local,
            sha1,
            size: 0,
            project_id: None,
            version_id: None,
        },
    )
    .await?;
    content_rows::set_entry_sides(&state.pool, id, path, sides).await?;
    Ok(())
}

/// Turn an addon on or off for one side. Off for the server, its file is `.disabled` and the
/// server's share still carries it; off for players, the server runs it and the share leaves it
/// out. A client-only file of the pack, which is not on the server, is placed when its server
/// side is turned on, and made client only again when it is turned off.
pub async fn set_content_side(
    state: &Engine,
    instance_id: &str,
    path: &str,
    side: ContentSide,
    enabled: bool,
) -> crate::Result<()> {
    server_only(state)?;
    let path = addon_path(path)?;
    let metadata = instance_metadata(state, instance_id).await?;
    server_first(&metadata)?;
    let _content = state.instance_locks.lock_content(instance_id).await;
    let folder = open_folder(&instance_dir(state, &metadata)).await?;
    if side == ContentSide::Server {
        if enabled
            && let Some(skipped) =
                content_rows::skipped_files(&state.pool, instance_id)
                    .await?
                    .into_iter()
                    .find(|file| file.relative_path == path)
        {
            place_client_only_file(state, &metadata, &folder, skipped).await?;
            changed(state, instance_id, &path);
            return Ok(());
        }
        if !enabled
            && let Some((row, sides)) =
                content_rows::tracked_file(&state.pool, instance_id, &path)
                    .await?
            && sides.server_override
            && row.by_pack()
        {
            return_client_only_file(state, &metadata, &folder, row).await?;
            changed(state, instance_id, &path);
            return Ok(());
        }
    }
    let (path, on_disk) = locate(&folder, &path)?;
    let mut sides = sides(state, instance_id, &path).await?;
    let requirement = if enabled {
        ContentRequirement::Required
    } else {
        ContentRequirement::Unsupported
    };
    match side {
        ContentSide::Server => {
            rename_enabled(&folder, &path, &on_disk, enabled)?;
            sides.server_requirement = requirement;
        }
        ContentSide::Player => sides.client_requirement = requirement,
    }
    save_sides(state, &metadata, &folder, &path, &sides).await?;
    changed(state, instance_id, &path);
    Ok(())
}

/// Mark an addon's sides as set by hand, or as following the file again.
pub async fn set_content_sides_unlocked(
    state: &Engine,
    instance_id: &str,
    path: &str,
    unlocked: bool,
) -> crate::Result<()> {
    server_only(state)?;
    let path = addon_path(path)?;
    let metadata = instance_metadata(state, instance_id).await?;
    server_first(&metadata)?;
    let _content = state.instance_locks.lock_content(instance_id).await;
    let folder = open_folder(&instance_dir(state, &metadata)).await?;
    let (path, _) = locate(&folder, &path)?;
    let mut sides = sides(state, instance_id, &path).await?;
    sides.unlocked = unlocked;
    save_sides(state, &metadata, &folder, &path, &sides).await
}

/// Install the pack the server follows again, placing every file of it that is missing or
/// changed.
pub async fn repair_content(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    server_only(state)?;
    repair_server_pack(state, instance_id).await
}

/// Place a client-only file of the pack on the server, from the pack it follows, and record
/// that someone turned it on for the server.
async fn place_client_only_file(
    state: &Engine,
    metadata: &InstanceMetadata,
    folder: &Arc<Folder>,
    skipped: SkippedFile,
) -> crate::Result<()> {
    let id = metadata.instance.id.as_str();
    let path = skipped.relative_path;
    let (sha1, size) = place_turned_on(state, metadata, folder, &path).await?;
    content_rows::upsert_tracked_file(
        &state.pool,
        id,
        &metadata.applied_content_set.id,
        &TrackedFile {
            relative_path: path.clone(),
            source_kind: pack_kind(&metadata.link),
            sha1,
            size,
            project_id: skipped.project_id,
            version_id: skipped.version_id,
        },
    )
    .await?;
    content_rows::set_entry_sides(
        &state.pool,
        id,
        &path,
        &EntrySides {
            unlocked: true,
            server_override: true,
            ..EntrySides::default()
        },
    )
    .await?;
    Ok(())
}

/// Turn the server side of a file [`place_client_only_file`] placed off: its file goes, enabled or
/// disabled, and it is client only again.
async fn return_client_only_file(
    state: &Engine,
    metadata: &InstanceMetadata,
    folder: &Folder,
    row: TrackedFile,
) -> crate::Result<()> {
    folder.remove(&row.relative_path)?;
    folder.remove(&format!("{}.disabled", row.relative_path))?;
    content_rows::return_to_client_only(
        &state.pool,
        &metadata.instance.id,
        &metadata.applied_content_set.id,
        &SkippedFile {
            relative_path: row.relative_path,
            sha1: row.sha1,
            size: row.size,
            project_id: row.project_id,
            version_id: row.version_id,
        },
    )
    .await
}
