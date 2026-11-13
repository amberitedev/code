//! A pack's client-only files on the server. Core leaves them off a server's disk and lists them
//! as not installed. Turning the server side of one on places it from the pack and records the
//! override, which pack updates keep; turning it off again removes the file and makes it client
//! only once more.

use std::sync::Arc;

use engine::sandbox::Folder;

use super::install_pack::{place_one, read_pack};
use crate::{
    installs::shared::SharedDirs,
    state::servers::{
        Content, EntrySides, Error, SkippedFile, TrackedFile,
        adapters::{filesystem::content_files::DISABLED, sqlite::content_rows},
    },
};

/// Place the pack's client-only file `skipped` on the server, downloaded or unpacked from the
/// pack the server follows, and record that someone turned it on for the server.
pub async fn place_client_only_file(
    content: &Content,
    server_id: &str,
    skipped: SkippedFile,
) -> Result<(), Error> {
    let pack = content_rows::get_pack(&content.db, server_id)
        .await?
        .ok_or_else(|| Error::Invalid("This server follows no pack.".into()))?;
    let archive_path = SharedDirs::new(content.paths.root()).pack(&pack.sha1);
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    let path = skipped.relative_path;
    let mut server_on =
        content_rows::get_server_overrides(&content.db, server_id).await?;
    server_on.paths.insert(path.clone());
    let (index, split) = read_pack(
        content,
        &archive_path,
        &server_on,
        folder.case_insensitive()?,
    )
    .await?;
    let want = split.wanted.get(&path).ok_or_else(|| {
        Error::Invalid(format!(
            "The pack cannot place {path} on a server: it is not on a host packs may download from."
        ))
    })?;
    place_one(
        content,
        server_id,
        &folder,
        &archive_path,
        &index,
        &path,
        want,
    )
    .await?;
    content_rows::upsert_tracked_file(
        &content.db,
        server_id,
        &TrackedFile {
            relative_path: path.clone(),
            source_kind: pack.set.source_kind,
            sha1: want.sha1.clone(),
            size: want.size,
            project_id: skipped.project_id,
            version_id: skipped.version_id,
            pending: None,
        },
    )
    .await?;
    content_rows::set_entry_sides(
        &content.db,
        server_id,
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
pub async fn return_client_only_file(
    content: &Content,
    server_id: &str,
    folder: &Folder,
    row: TrackedFile,
) -> Result<(), Error> {
    folder.remove(&row.relative_path)?;
    folder.remove(&format!("{}{DISABLED}", row.relative_path))?;
    content_rows::return_to_client_only(
        &content.db,
        server_id,
        &SkippedFile {
            relative_path: row.relative_path,
            sha1: row.sha1,
            size: row.size,
            project_id: row.project_id,
            version_id: row.version_id,
        },
    )
    .await?;
    Ok(())
}
