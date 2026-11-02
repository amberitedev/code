//! The pack files the owner edited since the pack placed them, and their answer for those a pack
//! update left alone.

use std::{collections::HashMap, sync::Arc};

use super::install_pack::{fetch_index_file, pack_staging, place, read_pack};
use crate::{
    api::content::{EditedFile, PendingChange},
    installs::shared::SharedDirs,
    state::servers::{
        Content, ContentSourceKind, Error, Pending, TrackedFile,
        adapters::{filesystem::folder::Folder, sqlite::content_rows},
        domain::pack::{self, Source},
    },
};

/// The pack files the owner changed or deleted since the pack placed them, with what a pack
/// update wants for each.
pub async fn list_edited_files(
    content: &Content,
    server_id: &str,
) -> Result<Vec<EditedFile>, Error> {
    let rows = content_rows::get_tracked_files(&content.db, server_id).await?;
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    Ok(tokio::task::spawn_blocking(move || {
        pack::edited(&rows, &|path| folder.sha1(path).ok().flatten())
            .into_iter()
            .map(|(row, on_disk)| EditedFile {
                path: row.relative_path.clone(),
                deleted: on_disk.is_none(),
                pending: row.pending.map(|pending| match pending {
                    Pending::Changed => PendingChange::Changed,
                    Pending::Removed => PendingChange::Removed,
                }),
            })
            .collect()
    })
    .await
    .map_err(std::io::Error::other)?)
}

/// The owner's answer for a file a pack update left alone: keep their version, or take the
/// pack's (replace).
pub async fn decide_edited_file(
    content: &Content,
    server_id: &str,
    path: &str,
    replace: bool,
) -> Result<(), Error> {
    let row = content_rows::get_tracked_file(&content.db, server_id, path)
        .await?
        .map(|(row, _)| row)
        .filter(|row| row.pending.is_some())
        .ok_or_else(|| {
            Error::NotFound(format!("{path} waits for no decision."))
        })?;
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    match (row.pending, replace) {
        (Some(Pending::Removed), false) => {
            let sha1 = folder.sha1(path)?.unwrap_or(row.sha1.clone());
            content_rows::upsert_tracked_file(
                &content.db,
                server_id,
                &TrackedFile {
                    source_kind: ContentSourceKind::Local,
                    sha1,
                    pending: None,
                    ..row
                },
            )
            .await?;
        }
        (Some(Pending::Removed), true) => {
            folder.remove(path)?;
            content_rows::remove_tracked_file(&content.db, server_id, path)
                .await?;
        }
        (_, replace) => {
            let pack = content_rows::get_pack(&content.db, server_id)
                .await?
                .ok_or_else(|| {
                    Error::Invalid("This server follows no pack.".into())
                })?;
            let archive_path =
                SharedDirs::new(content.paths.root()).pack(&pack.sha1);
            let (index, wanted, _) =
                read_pack(content, &archive_path, folder.case_insensitive()?)
                    .await?;
            let want = wanted.get(path).ok_or_else(|| {
                Error::NotFound(format!("The pack has no {path}."))
            })?;
            if replace {
                let mut staged = HashMap::new();
                if let Source::Download(n) = want.source {
                    let staging = pack_staging(content, server_id);
                    tokio::fs::create_dir_all(&staging).await?;
                    let target = staging.join(n.to_string());
                    fetch_index_file(
                        content,
                        &index.files[n],
                        &target,
                        &|_| {},
                    )
                    .await?;
                    staged.insert(n, target);
                }
                let placed =
                    place(&folder, &archive_path, path, want, &staged).await;
                let _ =
                    tokio::fs::remove_dir_all(pack_staging(content, server_id))
                        .await;
                placed?;
            }
            content_rows::upsert_tracked_file(
                &content.db,
                server_id,
                &TrackedFile {
                    source_kind: pack.set.source_kind,
                    sha1: want.sha1.clone(),
                    size: want.size,
                    pending: None,
                    ..row
                },
            )
            .await?;
        }
    }
    Ok(())
}
