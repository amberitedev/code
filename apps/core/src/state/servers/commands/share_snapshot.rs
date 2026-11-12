//! What a push of a server-first server would send now, read from the server as the share
//! diff and the version builder take it (`sharing_client`). The share routes (`api::share`), the
//! push's history record and `needs_update` compare versions with it.

use std::{collections::HashMap, sync::Arc};

use contracts::backend::sharing::{ExternalFile, VersionContent};
use modrinth_content_management::{
    CommonExternalFilePolicy, ContentSetConfiguration, ContentType,
    LoaderReference,
};
use sharing_client::{
    diff::{CarriedFile, DiffOptions, Disabled, ShareContent, VersionName},
    install::ModpackFile,
    push,
    rules::{CONFIG_FILE_TYPE, ShareScope},
};

use crate::{
    installs::ModLoader,
    servers::ServerError,
    state::servers::{
        Content, Error,
        adapters::{
            filesystem::{
                content_files,
                folder::{Folder, Sha1Cache},
            },
            history::History,
            network::modrinth::ModrinthError,
            sqlite::{content_rows, server_rows},
        },
        domain::server_share::{content_rows, pack_file, removed},
        modrinth_pack,
    },
};

/// Where a server's shared files come from.
pub const SCOPE: ShareScope = ShareScope::Server;

/// What a push of the server would send now.
pub struct Snapshot {
    /// The server's name, which the share takes.
    pub name: String,
    pub folder: Arc<Folder>,
    /// Core's SHA-1s of unchanged files, so comparing with a push hashes only what changed.
    pub sha1s: Arc<Sha1Cache>,
    pub content: VersionContent,
    /// The files the push carries, as the server has them.
    pub carried: Vec<push::ContentRow>,
    /// The files its modpack placed, by which the share diff splits a version on that modpack.
    pub pack_files: Vec<ModpackFile>,
}

pub async fn snapshot(content: &Content, id: &str) -> Result<Snapshot, Error> {
    let server = server_rows::get(&content.db, id)
        .await?
        .ok_or_else(|| Error::Server(ServerError::NotFound(id.to_owned())))?;
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    let (Some(loader), Some(game_version)) = (loader, server.game_version)
    else {
        return Err(Error::Invalid(
            "Install a loader on this server before sharing it.".into(),
        ));
    };
    // A server without a client loader is shared as vanilla, which players install as Fabric so
    // they can add their own mods.
    let (shared_loader, loader_version) = match loader {
        ModLoader::Fabric => ("fabric", server.loader_version),
        ModLoader::Quilt => ("quilt", server.loader_version),
        ModLoader::Forge => ("forge", server.loader_version),
        ModLoader::NeoForge => ("neoforge", server.loader_version),
        ModLoader::Vanilla | ModLoader::Paper => ("vanilla", None),
    };
    let folder = Arc::new(Folder::open(&content.paths.server(id))?);
    let listed = content_files::scan(&folder, Some(loader));
    let mut rows = HashMap::new();
    let mut entry_sides = HashMap::new();
    for (row, sides) in
        content_rows::get_tracked_files_with_sides(&content.db, id).await?
    {
        entry_sides.insert(row.relative_path.clone(), sides);
        rows.insert(row.relative_path.clone(), row);
    }
    // A pack uploaded as a file is not on Modrinth, so its files are shared one by one.
    let modpack_id =
        content_rows::get_pack(&content.db, id)
            .await?
            .and_then(|pack| {
                modrinth_pack(&pack.link)
                    .map(|(_, version_id)| version_id.to_owned())
            });
    let pack_files = rows
        .values()
        .filter(|row| row.by_pack())
        .map(pack_file)
        .collect::<Vec<_>>();
    let read = push::read_content(
        content_rows(&listed, &rows, &entry_sides, modpack_id.is_some()),
        if modpack_id.is_some() {
            &pack_files
        } else {
            &[]
        },
    );
    let removed_files = if modpack_id.is_some() {
        let level = folder.level_name();
        let dirs = [ContentType::Mod, ContentType::DataPack]
            .map(|kind| content_files::kind_folder(kind, &level))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        removed(&listed, &rows, &entry_sides, &dirs)
    } else {
        Vec::new()
    };
    // The files are staged when a push sends them; the builder decides the content.
    let (version, _) = push::build_version(
        SCOPE,
        push::VersionInput {
            modrinth_ids: read.modrinth_ids,
            modpack_id,
            removed_files,
            game_version,
            loader: shared_loader.to_owned(),
            loader_version,
            files: Vec::new(),
        },
    )
    .map_err(|error| Error::Other(color_eyre::eyre::eyre!("{error}")))?;
    Ok(Snapshot {
        name: server.name,
        folder,
        sha1s: Arc::clone(&content.sha1s),
        content: version,
        carried: read.carried,
        pack_files,
    })
}

/// A push compares every file it sends by SHA-1, which both sides know.
pub fn push_options() -> DiffOptions {
    DiffOptions {
        common_external_files: CommonExternalFilePolicy::AssumeUnchanged,
        disabled: Disabled::default(),
    }
}

/// The share history this Core keeps.
pub fn history_store(content: &Content) -> History<'_> {
    History {
        store: &content.content_store,
        db: &content.db,
    }
}

pub fn configuration(content: &VersionContent) -> ContentSetConfiguration {
    ContentSetConfiguration {
        modpack_version_id: content.modpack_id.clone(),
        game_version: content.game_version.clone(),
        loader: LoaderReference {
            name: content.loader.clone(),
            version: Some(content.loader_version.clone()),
        },
    }
}

/// A version, `content` with `files`, as the share diff reads it. A version on the server's own
/// modpack is split by the files that modpack placed.
pub fn version_content(
    snapshot: &Snapshot,
    content: &VersionContent,
    files: &[ExternalFile],
) -> Result<ShareContent, Error> {
    let pack = if content.modpack_id.is_some()
        && content.modpack_id == snapshot.content.modpack_id
    {
        snapshot.pack_files.clone()
    } else {
        Vec::new()
    };
    ShareContent::of(
        content,
        files.iter().map(CarriedFile::from),
        pack,
        configuration(content),
    )
    .map_err(|error| Error::Other(color_eyre::eyre::eyre!("{error}")))
}

/// A version (`content` with `files`), and what a push of the server sends now, as the share
/// diff reads them.
pub async fn share_contents(
    snapshot: &Snapshot,
    content: &VersionContent,
    files: &[ExternalFile],
) -> Result<(ShareContent, ShareContent), Error> {
    let before = version_content(snapshot, content, files)?;
    // A push sends the server's addons as they are on disk, and every config still shared as
    // the server has it, so one it no longer has leaves the share.
    let local = snapshot
        .carried
        .iter()
        .map(|row| {
            (
                row.file_type.clone(),
                row.file_name.clone(),
                row.path.clone(),
            )
        })
        .chain(
            files
                .iter()
                .filter(|file| file.file_type == CONFIG_FILE_TYPE)
                .map(|file| {
                    (
                        CONFIG_FILE_TYPE.to_owned(),
                        file.file_name.clone(),
                        file.file_name.clone(),
                    )
                }),
        )
        .collect::<Vec<_>>();
    let folder = Arc::clone(&snapshot.folder);
    let sha1s = Arc::clone(&snapshot.sha1s);
    let local = tokio::task::spawn_blocking(move || {
        local
            .into_iter()
            .map(|(file_type, file_name, on_disk)| {
                let sha1 = folder.sha1_cached(&on_disk, &sha1s).ok().flatten();
                (file_type, file_name, sha1)
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(std::io::Error::other)?;
    let after = ShareContent::of(
        &snapshot.content,
        local
            .iter()
            .map(|(file_type, file_name, sha1)| CarriedFile {
                file_type,
                file_name,
                sha1: sha1.as_deref(),
            }),
        snapshot.pack_files.clone(),
        configuration(&snapshot.content),
    )
    .map_err(|error| Error::Other(color_eyre::eyre::eyre!("{error}")))?;
    Ok((before, after))
}

/// Modrinth's names through Core's own client.
pub struct Names<'a>(pub &'a Content);

impl sharing_client::diff::Names for Names<'_> {
    type Error = ModrinthError;

    async fn versions(
        &self,
        ids: &[String],
    ) -> Result<Vec<VersionName>, ModrinthError> {
        Ok(self
            .0
            .modrinth
            .versions_by_id(ids)
            .await?
            .into_iter()
            .map(|version| VersionName {
                id: version.id,
                project_id: version.project_id,
                name: version.version_number.clone(),
                version_number: version.version_number,
            })
            .collect())
    }

    async fn projects(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, ModrinthError> {
        Ok(self
            .0
            .modrinth
            .projects(ids)
            .await?
            .into_iter()
            .map(|project| (project.id, project.title))
            .collect())
    }
}
