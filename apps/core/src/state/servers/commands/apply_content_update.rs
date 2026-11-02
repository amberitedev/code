//! Updating one addon to another version of its Modrinth project.

use std::{collections::HashSet, sync::Arc};

use bedrock::content::least_stable;

use super::apply_content_install::{place_version, resolve, target};
use crate::state::servers::{
    Content, ContentSourceKind, Error, TrackedFile,
    adapters::{
        filesystem::{
            content_files::{DISABLED, find, kind_folder},
            folder::Folder,
        },
        sqlite::content_rows,
    },
};

/// Update an addon to `version_id`, or its newest version for this server.
pub async fn update_project(
    content: &Content,
    server_id: &str,
    filename: &str,
    version_id: Option<&str>,
) -> Result<(), Error> {
    let target = target(content, server_id).await?;
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    let listed = find(&folder, Some(target.loader), filename)
        .ok_or_else(|| Error::NotFound(format!("There is no {filename}.")))?;
    let row =
        content_rows::get_tracked_file(&content.db, server_id, &listed.path)
            .await?
            .map(|(row, _)| row);
    // The version installed now: the one recorded, or the one Modrinth knows by its hash. A
    // recorded one Modrinth no longer answers for is unknown; the row still names the project.
    let installed = match row.as_ref().and_then(|row| row.version_id.as_deref())
    {
        Some(id) => content.modrinth.version(id).await.ok(),
        None => {
            let on_disk = if listed.disabled {
                format!("{}{DISABLED}", listed.path)
            } else {
                listed.path.clone()
            };
            let sha1 = folder.sha1(&on_disk)?.ok_or_else(|| {
                Error::NotFound(format!("There is no {filename}."))
            })?;
            content
                .modrinth
                .by_hashes(std::slice::from_ref(&sha1))
                .await?
                .remove(&sha1)
        }
    };
    let project = row
        .as_ref()
        .and_then(|row| row.project_id.clone())
        .or_else(|| {
            installed.as_ref().map(|version| version.project_id.clone())
        })
        .ok_or_else(|| {
            Error::Invalid(format!("Modrinth does not know {filename}."))
        })?;
    let version_id = match version_id {
        Some(id) => id.to_owned(),
        None => {
            // A beta someone installed keeps getting betas.
            let channel =
                installed.as_ref().map_or(target.channel, |version| {
                    least_stable(target.channel, version.version_type)
                });
            resolve(
                content,
                &target,
                &project,
                None,
                listed.kind,
                HashSet::new(),
                channel,
            )
            .await?
            .primary
            .version_id
        }
    };
    let version = content.modrinth.version(&version_id).await?;
    let rows = content_rows::get_tracked_files(&content.db, server_id).await?;
    let mut rows = rows;
    // The file being replaced counts as this project's even before Modrinth said so.
    if let Some(row) =
        rows.iter_mut().find(|row| row.relative_path == listed.path)
    {
        row.project_id = Some(project.clone());
    } else {
        rows.push(TrackedFile {
            relative_path: listed.path.clone(),
            source_kind: ContentSourceKind::Local,
            sha1: String::new(),
            size: 0,
            project_id: Some(project.clone()),
            version_id: None,
            pending: None,
        });
    }
    let dir = kind_folder(listed.kind, &folder.level_name())?;
    place_version(content, server_id, &folder, &dir, &version, &rows).await
}
