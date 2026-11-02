//! Changing an addon already on a server: removing it, turning it on or off, and its sides.

use bedrock::state::ContentRequirement;
use modrinth_content_management::ContentType;

use crate::state::servers::{
    Content, ContentSourceKind, EntrySides, Error, TrackedFile,
    adapters::{
        filesystem::{
            content_files::{DISABLED, locate, rename_enabled},
            folder::Folder,
        },
        sqlite::content_rows,
    },
};

/// Delete an addon. A pack file's row stays, so a pack update knows the owner removed it.
pub async fn remove_project(
    content: &Content,
    server_id: &str,
    kind: ContentType,
    filename: &str,
) -> Result<(), Error> {
    let folder = Folder::open(&content.paths.server(server_id))?;
    let (path, on_disk) = locate(&folder, kind, filename)?;
    folder.remove(&on_disk)?;
    match content_rows::get_tracked_file(&content.db, server_id, &path).await? {
        Some((row, _)) if row.by_pack() => {
            content_rows::set_entry_sides(
                &content.db,
                server_id,
                &path,
                &EntrySides::default(),
            )
            .await?;
        }
        Some(_) => {
            content_rows::remove_tracked_file(&content.db, server_id, &path)
                .await?;
        }
        None => {}
    }
    Ok(())
}

/// Disable an addon by renaming it to `.disabled`, or enable it again. Either way it is no longer
/// off for the server alone: disabled it is off for both sides.
pub async fn set_enabled(
    content: &Content,
    server_id: &str,
    kind: ContentType,
    filename: &str,
    enabled: bool,
) -> Result<(), Error> {
    let folder = Folder::open(&content.paths.server(server_id))?;
    let (path, on_disk) = locate(&folder, kind, filename)?;
    let mut sides = sides(content, server_id, &path).await?;
    sides.enabled = enabled;
    sides.server_requirement = ContentRequirement::Required;
    rename_enabled(&folder, &path, &on_disk, enabled)?;
    save_sides(content, server_id, &folder, &path, &sides).await
}

/// The sides recorded for the addon at its enabled `path`; on for both when none are.
async fn sides(
    content: &Content,
    server_id: &str,
    path: &str,
) -> Result<EntrySides, Error> {
    Ok(content_rows::get_tracked_file(&content.db, server_id, path)
        .await?
        .map(|(_, sides)| sides)
        .unwrap_or_default())
}

/// Record the sides of the addon at its enabled `path`. A file Core does not track yet is
/// tracked as someone's on the server, unless its sides are the default, which needs no record.
async fn save_sides(
    content: &Content,
    server_id: &str,
    folder: &Folder,
    path: &str,
    sides: &EntrySides,
) -> Result<(), Error> {
    if content_rows::set_entry_sides(&content.db, server_id, path, sides)
        .await?
        || *sides == EntrySides::default()
    {
        return Ok(());
    }
    let on_disk = if folder.exists(path) {
        path.to_owned()
    } else {
        format!("{path}{DISABLED}")
    };
    let sha1 = folder
        .sha1(&on_disk)?
        .ok_or_else(|| Error::NotFound(format!("There is no {path}.")))?;
    content_rows::upsert_tracked_file(
        &content.db,
        server_id,
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
    content_rows::set_entry_sides(&content.db, server_id, path, sides).await?;
    Ok(())
}

/// A side an addon can be turned off for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Server,
    Player,
}

/// Turn an addon on or off for one side. Off for the server, its file is `.disabled` and the
/// server's share still carries it; off for players, the server runs it and the share leaves it
/// out.
pub async fn set_side(
    content: &Content,
    server_id: &str,
    kind: ContentType,
    filename: &str,
    side: Side,
    enabled: bool,
) -> Result<(), Error> {
    let folder = Folder::open(&content.paths.server(server_id))?;
    let (path, on_disk) = locate(&folder, kind, filename)?;
    let mut sides = sides(content, server_id, &path).await?;
    let requirement = if enabled {
        ContentRequirement::Required
    } else {
        ContentRequirement::Unsupported
    };
    match side {
        Side::Server => {
            rename_enabled(&folder, &path, &on_disk, enabled)?;
            sides.server_requirement = requirement;
        }
        Side::Player => sides.client_requirement = requirement,
    }
    save_sides(content, server_id, &folder, &path, &sides).await
}

/// Mark an addon's sides as set by hand, or as following the file again.
pub async fn set_unlocked(
    content: &Content,
    server_id: &str,
    kind: ContentType,
    filename: &str,
    unlocked: bool,
) -> Result<(), Error> {
    let folder = Folder::open(&content.paths.server(server_id))?;
    let (path, _) = locate(&folder, kind, filename)?;
    let mut sides = sides(content, server_id, &path).await?;
    sides.unlocked = unlocked;
    save_sides(content, server_id, &folder, &path, &sides).await
}
