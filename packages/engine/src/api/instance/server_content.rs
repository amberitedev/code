//! Server content the App has no use for: which sides an addon is on for, the pack's client-only
//! files, and the pack files the owner edited. Only a server engine answers these.

use std::sync::Arc;

use contracts::Side;
use serde::{Deserialize, Serialize};

use crate::Engine;
use crate::api::pack::server::{
    self, EntrySides, SkippedFile, TrackedFile, project_type_of,
};
use crate::api::pack::server_install::{
    self, instance_dir, metadata, open_folder, pack_kind, place_one,
    read_kept_pack,
};
use crate::state::instances::adapters::sqlite::server_rows;
use crate::state::{
    ContentRequirement, ContentSourceKind, InstanceLink, InstanceMetadata,
    PendingChange,
};
use crate::util::sandbox::{self, Folder};

/// A side an addon can be turned off for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ContentSide {
    Server,
    Player,
}

/// A file of the pack the owner edited since the pack placed it, or deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct EditedFile {
    /// Relative to the server folder, with forward slashes.
    pub path: String,
    /// The owner deleted it (or disabled it) rather than changed it.
    pub deleted: bool,
    /// What the last pack update wants for it; null when nothing waits on a decision.
    pub pending: Option<PendingChange>,
}

fn server_only(state: &Engine) -> crate::Result<()> {
    if state.side != Side::Server {
        return Err(crate::ErrorKind::InputError(
            "only a server engine has this".to_string(),
        )
        .into());
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
    project_type_of(&path)
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
    state.events.send(crate::event::Event::FilesChanged {
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
    Ok(server_rows::tracked_file(&state.pool, instance_id, path)
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
    if server_rows::set_entry_sides(&state.pool, id, path, sides).await?
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
    server_rows::upsert_tracked_file(
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
            pending: None,
        },
    )
    .await?;
    server_rows::set_entry_sides(&state.pool, id, path, sides).await?;
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
    let metadata = metadata(state, instance_id).await?;
    server_first(&metadata)?;
    let _content = state.instance_locks.lock_content(instance_id).await;
    let folder = open_folder(&instance_dir(state, &metadata)).await?;
    if side == ContentSide::Server {
        if enabled
            && let Some(skipped) =
                server_rows::skipped_files(&state.pool, instance_id)
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
                server_rows::tracked_file(&state.pool, instance_id, &path)
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
    let metadata = metadata(state, instance_id).await?;
    server_first(&metadata)?;
    let _content = state.instance_locks.lock_content(instance_id).await;
    let folder = open_folder(&instance_dir(state, &metadata)).await?;
    let (path, _) = locate(&folder, &path)?;
    let mut sides = sides(state, instance_id, &path).await?;
    sides.unlocked = unlocked;
    save_sides(state, &metadata, &folder, &path, &sides).await
}

/// Nothing to do: a server engine never guesses sides, so it never asks for a review.
pub async fn complete_side_review(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    server_only(state)?;
    metadata(state, instance_id).await.map(|_| ())
}

/// Place every file of the pack the server follows that is missing.
pub async fn repair_content(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    server_only(state)?;
    server_install::repair(state, instance_id).await
}

/// Place a client-only file of the pack on the server, from the pack it follows, and record
/// that someone turned it on for the server.
pub async fn place_client_only(
    state: &Engine,
    instance_id: &str,
    path: &str,
) -> crate::Result<()> {
    set_content_side(state, instance_id, path, ContentSide::Server, true).await
}

async fn place_client_only_file(
    state: &Engine,
    metadata: &InstanceMetadata,
    folder: &Arc<Folder>,
    skipped: SkippedFile,
) -> crate::Result<()> {
    let id = metadata.instance.id.as_str();
    let path = skipped.relative_path;
    let mut server_on = server_rows::server_overrides(&state.pool, id).await?;
    server_on.paths.insert(path.clone());
    let (index, split) =
        read_kept_pack(state, metadata, folder, &server_on).await?;
    let want = split.wanted.get(&path).ok_or_else(|| {
        refused(format!(
            "The pack cannot place {path} on a server: it is not on a host packs may download from."
        ))
    })?;
    place_one(state, metadata, folder, &index, &path, want).await?;
    server_rows::upsert_tracked_file(
        &state.pool,
        id,
        &metadata.applied_content_set.id,
        &TrackedFile {
            relative_path: path.clone(),
            source_kind: pack_kind(&metadata.link),
            sha1: want.sha1.clone(),
            size: want.size,
            project_id: skipped.project_id,
            version_id: skipped.version_id,
            pending: None,
        },
    )
    .await?;
    server_rows::set_entry_sides(
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
    server_rows::return_to_client_only(
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

/// The pack files the owner changed or deleted since the pack placed them, with what a pack
/// update wants for each.
pub async fn edited_files(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<Vec<EditedFile>> {
    server_only(state)?;
    let metadata = metadata(state, instance_id).await?;
    let rows: Vec<TrackedFile> =
        server_rows::tracked_files(&state.pool, instance_id)
            .await?
            .into_iter()
            .map(|(row, _)| row)
            .collect();
    let folder = open_folder(&instance_dir(state, &metadata)).await?;
    Ok(tokio::task::spawn_blocking(move || {
        server::edited(&rows, &|path| folder.sha1(path).ok().flatten())
            .into_iter()
            .map(|(row, on_disk)| EditedFile {
                path: row.relative_path.clone(),
                deleted: on_disk.is_none(),
                pending: row.pending,
            })
            .collect()
    })
    .await?)
}

/// The owner's answer for a file a pack update left alone: keep their version, or take the
/// pack's (`replace`).
pub async fn decide_edited(
    state: &Engine,
    instance_id: &str,
    path: &str,
    replace: bool,
) -> crate::Result<()> {
    server_only(state)?;
    let metadata = metadata(state, instance_id).await?;
    let _content = state.instance_locks.lock_content(instance_id).await;
    let row = server_rows::tracked_file(&state.pool, instance_id, path)
        .await?
        .map(|(row, _)| row)
        .filter(|row| row.pending.is_some())
        .ok_or_else(|| refused(format!("{path} waits for no decision.")))?;
    let set_id = metadata.applied_content_set.id.as_str();
    let folder = open_folder(&instance_dir(state, &metadata)).await?;
    match (row.pending, replace) {
        (Some(PendingChange::Removed), false) => {
            let sha1 = folder.sha1(path)?.unwrap_or(row.sha1.clone());
            server_rows::upsert_tracked_file(
                &state.pool,
                instance_id,
                set_id,
                &TrackedFile {
                    source_kind: ContentSourceKind::Local,
                    sha1,
                    pending: None,
                    ..row
                },
            )
            .await?;
        }
        (Some(PendingChange::Removed), true) => {
            folder.remove(path)?;
            server_rows::remove_tracked_file(&state.pool, instance_id, path)
                .await?;
        }
        (_, replace) => {
            let server_on =
                server_rows::server_overrides(&state.pool, instance_id).await?;
            let (index, split) =
                read_kept_pack(state, &metadata, &folder, &server_on).await?;
            let want = split
                .wanted
                .get(path)
                .ok_or_else(|| refused(format!("The pack has no {path}.")))?;
            if replace {
                place_one(state, &metadata, &folder, &index, path, want)
                    .await?;
            }
            server_rows::upsert_tracked_file(
                &state.pool,
                instance_id,
                set_id,
                &TrackedFile {
                    source_kind: pack_kind(&metadata.link),
                    sha1: want.sha1.clone(),
                    size: want.size,
                    pending: None,
                    ..row
                },
            )
            .await?;
        }
    }
    changed(state, instance_id, path);
    Ok(())
}
