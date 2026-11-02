//! Adding content to a server: a Modrinth project with the dependencies nothing provides yet, or a
//! file the App uploaded.
//!
//! Which version to install and which dependencies come with it is
//! `modrinth_content_management::resolve_content`'s, as in the App: the update channel falls back
//! one step at a time when nothing on it fits, and a dependency it cannot place is reported in
//! the log instead of failing the add.

use std::{collections::HashSet, path::Path, sync::Arc};

use bedrock::state::ContentRequirement;
use modrinth_content_management::{
    ContentType, ResolutionPreferences, ResolveContentPlan,
    ResolveContentRequest, SkippedReason, VersionType, resolve_content,
};
use tracing::warn;

use super::sync_content_files::identify;
use crate::{
    installs::{
        ModLoader,
        download::{self, Hash},
    },
    state::servers::{
        Content, ContentSourceKind, EntrySides, Error, TrackedFile,
        adapters::{
            filesystem::{
                archive,
                content_files::{DISABLED, kind_folder},
                folder::{self, Folder},
            },
            network::{
                content_download, hosts,
                modrinth::{self, Version},
            },
            sqlite::{content_rows, server_rows},
        },
    },
};

/// What a server runs, for picking versions.
pub struct Target {
    pub loader: ModLoader,
    pub game_version: String,
    /// The server's update channel.
    pub channel: VersionType,
}

pub async fn target(
    content: &Content,
    server_id: &str,
) -> Result<Target, Error> {
    let server =
        server_rows::get(&content.db, server_id)
            .await?
            .ok_or_else(|| {
                Error::NotFound(format!("There is no server {server_id}."))
            })?;
    match (
        server.loader.as_deref().and_then(ModLoader::parse),
        server.game_version,
    ) {
        (Some(loader), Some(game_version)) => Ok(Target {
            loader,
            game_version,
            channel: server.update_channel,
        }),
        _ => Err(Error::Invalid(
            "Install a loader on this server first.".into(),
        )),
    }
}

/// The Modrinth loaders content of `kind` is picked by on this server. A modpack's are the
/// server's loader's, as for mods.
pub fn loaders(target: &Target, kind: ContentType) -> Vec<&'static str> {
    match kind {
        ContentType::DataPack => vec!["datapack"],
        _ => modrinth::loaders_for(target.loader),
    }
}

/// What a project is picked as when the App names no kind: plugins on Paper, datapacks on
/// Vanilla, mods otherwise, which `resolve_content` lets match datapack versions too.
fn default_kind(loader: ModLoader) -> ContentType {
    match loader {
        ModLoader::Paper => ContentType::Plugin,
        ModLoader::Vanilla => ContentType::DataPack,
        _ => ContentType::Mod,
    }
}

/// The kind a version is, from the loaders it lists.
fn kind_of(version: &Version, target: &Target) -> ContentType {
    let has = |name: &str| version.loaders.iter().any(|loader| loader == name);
    if has("datapack")
        && (target.loader == ModLoader::Vanilla || version.loaders.len() == 1)
    {
        ContentType::DataPack
    } else if target.loader == ModLoader::Paper {
        ContentType::Plugin
    } else {
        ContentType::Mod
    }
}

/// What `resolve_content` picks for `project_id` on this server: `version_id`, or the newest
/// version of `kind` on `channel`, and the required dependencies none of `existing` provide.
pub async fn resolve(
    content: &Content,
    target: &Target,
    project_id: &str,
    version_id: Option<&str>,
    kind: ContentType,
    existing: HashSet<String>,
    channel: VersionType,
) -> Result<ResolveContentPlan, Error> {
    let plan = resolve_content(
        content.modrinth.clone(),
        ResolveContentRequest {
            project_id: project_id.to_owned(),
            version_id: version_id.map(str::to_owned),
            content_type: kind,
            selected: ResolutionPreferences::default(),
            target: ResolutionPreferences {
                game_versions: vec![target.game_version.clone()],
                loaders: loaders(target, kind)
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            },
            existing_project_ids: existing.into_iter().collect(),
            update_channel: Some(channel),
        },
    )
    .await?;
    for skipped in &plan.skipped {
        if !matches!(
            skipped.reason,
            SkippedReason::AlreadyInstalled | SkippedReason::DuplicateProject
        ) {
            warn!(
                project = %project_id,
                dependency = %skipped.project_id,
                reason = ?skipped.reason,
                "a dependency was left out"
            );
        }
    }
    Ok(plan)
}

/// Add a Modrinth project, and the dependencies it requires that nothing on the server provides
/// yet: a project the pack or anyone already put there counts. An older file of the same project
/// is replaced.
pub async fn add_project(
    content: &Content,
    server_id: &str,
    project_id: &str,
    version_id: Option<&str>,
    kind: Option<ContentType>,
) -> Result<(), Error> {
    let target = target(content, server_id).await?;
    let rows = content_rows::get_tracked_files(&content.db, server_id).await?;
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    let mut existing = provided(&rows, &|path| folder.exists(path));
    existing.remove(project_id);
    let plan = resolve(
        content,
        &target,
        project_id,
        version_id,
        kind.unwrap_or(default_kind(target.loader)),
        existing,
        target.channel,
    )
    .await?;
    let primary = content.modrinth.version(&plan.primary.version_id).await?;
    let kind = kind.unwrap_or_else(|| kind_of(&primary, &target));
    let mut install = vec![primary];
    for dependency in &plan.dependencies {
        install.push(content.modrinth.version(&dependency.version_id).await?);
    }
    let dir = kind_folder(kind, &folder.level_name())?;
    for version in &install {
        place_version(content, server_id, &folder, &dir, version, &rows)
            .await?;
    }
    Ok(())
}

/// The projects this server already has: those of recorded files that are enabled and on disk.
/// A disabled file's row keeps its enabled path, so it is not there.
fn provided(
    rows: &[TrackedFile],
    exists: &dyn Fn(&str) -> bool,
) -> HashSet<String> {
    rows.iter()
        .filter(|row| exists(&row.relative_path))
        .filter_map(|row| row.project_id.clone())
        .collect()
}

/// Download a version's file into `dir`, replacing an older file of the same project, and record
/// it as added. The sides of the file it replaces go to the new one.
pub async fn place_version(
    content: &Content,
    server_id: &str,
    folder: &Arc<Folder>,
    dir: &str,
    version: &Version,
    rows: &[TrackedFile],
) -> Result<(), Error> {
    let file = version.primary().ok_or_else(|| {
        Error::Invalid(format!("Version {} has no file.", version.id))
    })?;
    let name = folder::clean(&file.filename)?;
    if name.contains('/') {
        return Err(Error::Invalid(format!(
            "{} is not a file name.",
            file.filename
        )));
    }
    let path = format!("{dir}/{name}");
    let url = reqwest::Url::parse(&file.url)
        .ok()
        .filter(|url| hosts::allowed(url, false))
        .ok_or_else(|| {
            Error::Invalid(format!("{} is not on an allowed host.", file.url))
        })?;
    let staging = content
        .paths
        .staging(server_id)
        .join(uuid::Uuid::new_v4().to_string());
    let hash = file
        .hashes
        .sha512
        .as_deref()
        .map_or(Hash::Sha1(&file.hashes.sha1), Hash::Sha512);
    content_download::fetch(
        &content.downloads,
        url.as_str(),
        &staging,
        hash,
        &download::quiet,
    )
    .await?;
    let old: Vec<TrackedFile> = rows
        .iter()
        .filter(|row| {
            row.project_id.as_deref() == Some(version.project_id.as_str())
                && row.relative_path.starts_with(&format!("{dir}/"))
                && row.relative_path != path
        })
        .cloned()
        .collect();
    let sides = content_rows::get_entry_sides(&content.db, server_id).await?;
    let moved = old
        .iter()
        .find_map(|row| sides.get(&row.relative_path).copied());
    // A file off for the server stays off when it is updated, since its sides move to the new
    // file and only count while the file is `.disabled`.
    let off_for_server = moved.is_some_and(|sides| {
        sides.server_requirement == ContentRequirement::Unsupported
    });
    let written = if off_for_server {
        format!("{path}{DISABLED}")
    } else {
        path.clone()
    };
    let placed = {
        let folder = Arc::clone(folder);
        let old = old.clone();
        let staged = staging.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            folder.write(&written, &mut std::fs::File::open(&staged)?)?;
            for row in &old {
                folder.remove(&row.relative_path)?;
                folder.remove(&format!("{}{DISABLED}", row.relative_path))?;
            }
            Ok(())
        })
        .await
        .map_err(std::io::Error::other)
    };
    let _ = tokio::fs::remove_file(&staging).await;
    placed??;
    for row in old {
        // A pack file's row stays, so the pack does not put it back; its sides moved on.
        if row.by_pack() {
            content_rows::set_entry_sides(
                &content.db,
                server_id,
                &row.relative_path,
                &EntrySides::default(),
            )
            .await?;
        } else {
            content_rows::remove_tracked_file(
                &content.db,
                server_id,
                &row.relative_path,
            )
            .await?;
        }
    }
    content_rows::upsert_tracked_file(
        &content.db,
        server_id,
        &TrackedFile {
            relative_path: path.clone(),
            source_kind: ContentSourceKind::Local,
            sha1: file.hashes.sha1.to_ascii_lowercase(),
            size: file.size,
            project_id: Some(version.project_id.clone()),
            version_id: Some(version.id.clone()),
            pending: None,
        },
    )
    .await?;
    if let Some(sides) = moved {
        content_rows::set_entry_sides(&content.db, server_id, &path, &sides)
            .await?;
    }
    Ok(())
}

/// Place a file uploaded from the App: `.jar` files are mods (plugins on Paper), `.zip` files
/// datapacks. An existing file of that name is replaced.
pub async fn add_uploaded_file(
    content: &Content,
    server_id: &str,
    filename: &str,
    staged: &Path,
) -> Result<(), Error> {
    let server =
        server_rows::get(&content.db, server_id)
            .await?
            .ok_or_else(|| {
                Error::NotFound(format!("There is no server {server_id}."))
            })?;
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    let name = folder::clean(filename)?;
    if name.contains('/') {
        return Err(Error::Invalid(format!("{filename} is not a file name.")));
    }
    let kind = if name.ends_with(".zip") {
        ContentType::DataPack
    } else if name.ends_with(".jar") {
        if loader == Some(ModLoader::Paper) {
            ContentType::Plugin
        } else {
            ContentType::Mod
        }
    } else {
        return Err(Error::Invalid(format!(
            "{filename} is neither a .jar nor a datapack .zip."
        )));
    };
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    let path = format!("{}/{name}", kind_folder(kind, &folder.level_name())?);
    let sha1 = {
        let folder = Arc::clone(&folder);
        let path = path.clone();
        let staged = staged.to_owned();
        tokio::task::spawn_blocking(move || -> std::io::Result<String> {
            let mut reader =
                archive::Hashing::new(std::fs::File::open(staged)?);
            folder.write(&path, &mut reader)?;
            Ok(reader.finish())
        })
        .await
        .map_err(std::io::Error::other)??
    };
    // Over a pack file, the row keeps what the pack placed, so the upload counts as the owner's
    // edit and a pack update leaves it alone.
    let pack_file =
        content_rows::get_tracked_file(&content.db, server_id, &path)
            .await?
            .is_some_and(|(row, _)| row.by_pack());
    if !pack_file {
        content_rows::upsert_tracked_file(
            &content.db,
            server_id,
            &TrackedFile {
                relative_path: path,
                source_kind: ContentSourceKind::Local,
                sha1,
                size: tokio::fs::metadata(staged).await?.len(),
                project_id: None,
                version_id: None,
                pending: None,
            },
        )
        .await?;
    }
    identify(content, server_id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_enabled_files_on_disk_provide_a_dependency() {
        let row = |path: &str, project: &str| TrackedFile {
            relative_path: path.into(),
            source_kind: ContentSourceKind::Local,
            sha1: "x".into(),
            size: 1,
            project_id: Some(project.into()),
            version_id: None,
            pending: None,
        };
        let rows = [
            row("mods/api.jar", "api"),
            // Disabled: on disk as mods/lib.jar.disabled.
            row("mods/lib.jar", "lib"),
            // Deleted.
            row("mods/gone.jar", "gone"),
        ];
        let on_disk = ["mods/api.jar", "mods/lib.jar.disabled"];
        assert_eq!(
            provided(&rows, &|path| on_disk.contains(&path)),
            HashSet::from(["api".to_owned()])
        );
    }
}
