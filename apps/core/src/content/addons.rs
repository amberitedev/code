//! Addons: mods, plugins, and datapacks, as files in their folders. A disabled addon is its file
//! with `.disabled` added; its row keeps the enabled path, so a pack update treats a disabled
//! pack file like one the owner removed and leaves it alone.

use std::{collections::HashSet, path::Path, sync::Arc};

use tracing::warn;

use super::{
    Content, Error,
    folder::{self, Folder},
    install::identify,
    modrinth::{self, Version},
    records::{self, ContentFile, Origin, SideToggle},
};
use crate::{
    api::content::AddonKind,
    installs::{
        ModLoader,
        download::{self, FileDownload, Hash},
    },
    servers::records as servers_records,
};

/// Added to an addon's file name to disable it.
pub const DISABLED: &str = ".disabled";
/// Dependencies followed for one add, however deep.
const MAX_DEPENDENCIES: usize = 64;

/// The folder addons of `kind` live in. Datapacks are inside the world; adding one on purpose is
/// the one way content goes there.
pub fn kind_folder(kind: AddonKind, level_name: &str) -> String {
    match kind {
        AddonKind::Mod => "mods".into(),
        AddonKind::Plugin => "plugins".into(),
        AddonKind::Datapack => format!("{level_name}/datapacks"),
        AddonKind::Shader => "shaderpacks".into(),
        AddonKind::Resourcepack => "resourcepacks".into(),
    }
}

/// The kinds a server with `loader` lists.
pub fn kinds(loader: Option<ModLoader>) -> Vec<AddonKind> {
    match loader {
        Some(ModLoader::Paper) => vec![AddonKind::Plugin, AddonKind::Datapack],
        Some(ModLoader::Vanilla) | None => vec![AddonKind::Datapack],
        Some(_) => vec![AddonKind::Mod, AddonKind::Datapack],
    }
}

/// One addon file on disk.
#[derive(Debug, Clone)]
pub struct Listed {
    /// The enabled path, which its row uses.
    pub path: String,
    pub kind: AddonKind,
    pub filename: String,
    pub disabled: bool,
    pub size: u64,
    pub created: Option<chrono::DateTime<chrono::Utc>>,
}

/// Every addon file of the server's kinds.
pub fn scan(folder: &Folder, loader: Option<ModLoader>) -> Vec<Listed> {
    let level = folder.level_name();
    let mut listed = Vec::new();
    for kind in kinds(loader) {
        let dir = kind_folder(kind, &level);
        let Ok(entries) =
            folder.open_dir(Some(&dir)).and_then(|dir| dir.entries())
        else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let (filename, disabled) = match name.strip_suffix(DISABLED) {
                Some(enabled) => (enabled.to_owned(), true),
                None => (name.clone(), false),
            };
            if !(filename.ends_with(".jar") || filename.ends_with(".zip")) {
                continue;
            }
            listed.push(Listed {
                path: format!("{dir}/{filename}"),
                kind,
                filename,
                disabled,
                size: meta.len(),
                created: meta
                    .created()
                    .or_else(|_| meta.modified())
                    .ok()
                    .map(|time| chrono::DateTime::from(time.into_std())),
            });
        }
    }
    listed.sort_by_key(|listed| listed.filename.to_lowercase());
    listed
}

/// What a server runs, for picking versions.
struct Target {
    loader: ModLoader,
    game_version: String,
}

async fn target(content: &Content, server_id: &str) -> Result<Target, Error> {
    let server = servers_records::get(&content.db, server_id)
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
        }),
        _ => Err(Error::Invalid(
            "Install a loader on this server first.".into(),
        )),
    }
}

/// The Modrinth loaders a `kind` is filtered by on this server.
fn loaders(target: &Target, kind: AddonKind) -> Vec<&'static str> {
    match kind {
        AddonKind::Datapack => vec!["datapack"],
        _ => modrinth::loaders_for(target.loader),
    }
}

/// The kind a version is, from the loaders it lists.
fn kind_of(version: &Version, target: &Target) -> AddonKind {
    let has = |name: &str| version.loaders.iter().any(|loader| loader == name);
    if has("datapack")
        && (target.loader == ModLoader::Vanilla || version.loaders.len() == 1)
    {
        AddonKind::Datapack
    } else if target.loader == ModLoader::Paper {
        AddonKind::Plugin
    } else {
        AddonKind::Mod
    }
}

/// The newest version of `project` that runs on this server.
async fn latest(
    content: &Content,
    target: &Target,
    project: &str,
    kind: Option<AddonKind>,
) -> Result<Version, Error> {
    let filter = kind.map_or_else(
        || modrinth::loaders_for(target.loader),
        |kind| loaders(target, kind),
    );
    content
        .modrinth
        .versions(project, &filter, Some(&target.game_version))
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| {
            Error::Invalid(format!(
                "{project} has no version for {} {}.",
                target.loader, target.game_version
            ))
        })
}

/// Add a Modrinth project, and the dependencies it requires that nothing on the server provides
/// yet: a project the pack or anyone already put there counts. An older file of the same project
/// is replaced.
pub async fn add(
    content: &Content,
    server_id: &str,
    project_id: &str,
    version_id: Option<&str>,
    kind: Option<AddonKind>,
) -> Result<(), Error> {
    let target = target(content, server_id).await?;
    let version = match version_id {
        Some(id) => content.modrinth.version(id).await?,
        None => latest(content, &target, project_id, kind).await?,
    };
    let kind = kind.unwrap_or_else(|| kind_of(&version, &target));
    let rows = records::files(&content.db, server_id).await?;
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    let mut provided = provided(&rows, &|path| folder.exists(path));
    provided.remove(&version.project_id);

    let mut install = vec![version];
    let mut index = 0;
    while index < install.len() && install.len() < MAX_DEPENDENCIES {
        let dependencies = install[index].dependencies.clone();
        provided.insert(install[index].project_id.clone());
        index += 1;
        for dependency in dependencies {
            if dependency.dependency_type != "required" {
                continue;
            }
            let dependency_version =
                match (&dependency.version_id, &dependency.project_id) {
                    (Some(version), _) => {
                        content.modrinth.version(version).await?
                    }
                    (None, Some(project)) if !provided.contains(project) => {
                        latest(content, &target, project, Some(kind)).await?
                    }
                    _ => continue,
                };
            if provided.contains(&dependency_version.project_id) {
                continue;
            }
            provided.insert(dependency_version.project_id.clone());
            install.push(dependency_version);
        }
    }

    let dir = kind_folder(kind, &folder.level_name());
    for version in &install {
        place_version(content, server_id, &folder, &dir, version, &rows)
            .await?;
    }
    Ok(())
}

/// The projects this server already has: those of recorded files that are enabled and on disk.
/// A disabled file's row keeps its enabled path, so it is not there.
fn provided(
    rows: &[ContentFile],
    exists: &dyn Fn(&str) -> bool,
) -> HashSet<String> {
    rows.iter()
        .filter(|row| exists(&row.path))
        .filter_map(|row| row.project_id.clone())
        .collect()
}

/// Download a version's file into `dir`, replacing an older file of the same project, and record
/// it as added.
async fn place_version(
    content: &Content,
    server_id: &str,
    folder: &Arc<Folder>,
    dir: &str,
    version: &Version,
    rows: &[ContentFile],
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
        .filter(|url| super::hosts::allowed(url, false))
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
    download::file(
        &content.downloads,
        url.as_str(),
        &staging,
        &FileDownload {
            hash,
            limit: Some(file.size.max(1)),
            progress: &download::quiet,
        },
    )
    .await?;
    let old: Vec<ContentFile> = rows
        .iter()
        .filter(|row| {
            row.project_id.as_deref() == Some(version.project_id.as_str())
                && row.path.starts_with(&format!("{dir}/"))
                && row.path != path
        })
        .cloned()
        .collect();
    // A file off for the server stays off when it is updated, since its toggle moves to the new
    // file and only counts while the file is `.disabled`.
    let toggles = records::toggles(&content.db, server_id).await?;
    let off_for_server = old.iter().any(|row| {
        toggles
            .iter()
            .any(|toggle| toggle.path == row.path && toggle.disabled_server)
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
                folder.remove(&row.path)?;
                folder.remove(&format!("{}{DISABLED}", row.path))?;
            }
            Ok(())
        })
        .await
        .map_err(std::io::Error::other)
    };
    let _ = tokio::fs::remove_file(&staging).await;
    placed??;
    for row in old {
        records::move_toggle(&content.db, server_id, &row.path, &path).await?;
        // A pack file's row stays, so the pack does not put it back.
        if row.origin == Origin::Added {
            records::remove(&content.db, server_id, &row.path).await?;
        }
    }
    records::put(
        &content.db,
        server_id,
        &ContentFile {
            path,
            origin: Origin::Added,
            sha1: file.hashes.sha1.to_ascii_lowercase(),
            project_id: Some(version.project_id.clone()),
            version_id: Some(version.id.clone()),
            pending: None,
        },
    )
    .await?;
    Ok(())
}

/// Where an addon is on disk: its enabled path, and the path of the file there now.
fn locate(
    folder: &Folder,
    kind: AddonKind,
    filename: &str,
) -> Result<(String, String), Error> {
    let name = folder::clean(filename.trim_end_matches(DISABLED))?;
    if name.contains('/') {
        return Err(Error::Invalid(format!("{filename} is not a file name.")));
    }
    let path = format!("{}/{name}", kind_folder(kind, &folder.level_name()));
    let disabled = format!("{path}{DISABLED}");
    if folder.exists(&path) {
        Ok((path.clone(), path))
    } else if folder.exists(&disabled) {
        Ok((path, disabled))
    } else {
        Err(Error::NotFound(format!("There is no {filename}.")))
    }
}

/// Delete an addon. A pack file's row stays, so a pack update knows the owner removed it.
pub async fn remove(
    content: &Content,
    server_id: &str,
    kind: AddonKind,
    filename: &str,
) -> Result<(), Error> {
    let folder = Folder::open(&content.paths.server(server_id))?;
    let (path, on_disk) = locate(&folder, kind, filename)?;
    folder.remove(&on_disk)?;
    records::remove_toggle(&content.db, server_id, &path).await?;
    if let Some(row) = records::file(&content.db, server_id, &path).await?
        && row.origin == Origin::Added
    {
        records::remove(&content.db, server_id, &path).await?;
    }
    Ok(())
}

/// Disable an addon by renaming it to `.disabled`, or enable it again. Either way it is no longer
/// off for the server alone: disabled it is off for both sides.
pub async fn set_enabled(
    content: &Content,
    server_id: &str,
    kind: AddonKind,
    filename: &str,
    enabled: bool,
) -> Result<(), Error> {
    let folder = Folder::open(&content.paths.server(server_id))?;
    let (path, on_disk) = locate(&folder, kind, filename)?;
    let mut toggle = toggle(content, server_id, &path).await?;
    toggle.disabled_server = false;
    rename_enabled(&folder, &path, &on_disk, enabled)?;
    records::set_toggle(&content.db, server_id, &toggle).await?;
    Ok(())
}

fn rename_enabled(
    folder: &Folder,
    path: &str,
    on_disk: &str,
    enabled: bool,
) -> Result<(), Error> {
    let wanted = if enabled {
        path.to_owned()
    } else {
        format!("{path}{DISABLED}")
    };
    if on_disk != wanted {
        folder.rename(on_disk, &wanted)?;
    }
    Ok(())
}

async fn toggle(
    content: &Content,
    server_id: &str,
    path: &str,
) -> Result<SideToggle, Error> {
    Ok(records::toggles(&content.db, server_id)
        .await?
        .into_iter()
        .find(|toggle| toggle.path == path)
        .unwrap_or_else(|| SideToggle {
            path: path.to_owned(),
            ..SideToggle::default()
        }))
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
    kind: AddonKind,
    filename: &str,
    side: Side,
    enabled: bool,
) -> Result<(), Error> {
    let folder = Folder::open(&content.paths.server(server_id))?;
    let (path, on_disk) = locate(&folder, kind, filename)?;
    let mut toggle = toggle(content, server_id, &path).await?;
    match side {
        Side::Server => {
            rename_enabled(&folder, &path, &on_disk, enabled)?;
            toggle.disabled_server = !enabled;
        }
        Side::Player => toggle.disabled_player = !enabled,
    }
    records::set_toggle(&content.db, server_id, &toggle).await?;
    Ok(())
}

/// Mark an addon's sides as set by hand, or as following the file again.
pub async fn set_unlocked(
    content: &Content,
    server_id: &str,
    kind: AddonKind,
    filename: &str,
    unlocked: bool,
) -> Result<(), Error> {
    let folder = Folder::open(&content.paths.server(server_id))?;
    let (path, _) = locate(&folder, kind, filename)?;
    let mut toggle = toggle(content, server_id, &path).await?;
    toggle.unlocked = unlocked;
    records::set_toggle(&content.db, server_id, &toggle).await?;
    Ok(())
}

/// Where an addon runs, from its file and its toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sides {
    /// Off for both sides.
    pub disabled: bool,
    pub disabled_server: bool,
    pub disabled_player: bool,
}

impl Sides {
    /// A `.disabled` file is off for the server alone only while its toggle says so; a file
    /// renamed back by hand runs again.
    pub fn of(listed: &Listed, toggle: Option<&SideToggle>) -> Self {
        let disabled_server = listed.disabled
            && toggle.is_some_and(|toggle| toggle.disabled_server);
        Self {
            disabled: listed.disabled && !disabled_server,
            disabled_server,
            disabled_player: toggle
                .is_some_and(|toggle| toggle.disabled_player),
        }
    }

    /// The server's share carries it, for players.
    pub fn for_players(self) -> bool {
        !self.disabled && !self.disabled_player
    }
}

/// Find an addon by file name in any folder this server lists.
pub fn find(
    folder: &Folder,
    loader: Option<ModLoader>,
    filename: &str,
) -> Option<Listed> {
    let name = filename.trim_end_matches(DISABLED);
    scan(folder, loader)
        .into_iter()
        .find(|listed| listed.filename == name)
}

/// Update an addon to `version_id`, or its newest version for this server.
pub async fn update(
    content: &Content,
    server_id: &str,
    filename: &str,
    version_id: Option<&str>,
) -> Result<(), Error> {
    let target = target(content, server_id).await?;
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    let listed = find(&folder, Some(target.loader), filename)
        .ok_or_else(|| Error::NotFound(format!("There is no {filename}.")))?;
    let row = records::file(&content.db, server_id, &listed.path).await?;
    let project = match row.as_ref().and_then(|row| row.project_id.clone()) {
        Some(project) => project,
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
                .map(|version| version.project_id)
                .ok_or_else(|| {
                    Error::Invalid(format!(
                        "Modrinth does not know {filename}."
                    ))
                })?
        }
    };
    let version = match version_id {
        Some(id) => content.modrinth.version(id).await?,
        None => latest(content, &target, &project, Some(listed.kind)).await?,
    };
    let rows = records::files(&content.db, server_id).await?;
    let mut rows = rows;
    // The file being replaced counts as this project's even before Modrinth said so.
    if let Some(row) = rows.iter_mut().find(|row| row.path == listed.path) {
        row.project_id = Some(project.clone());
    } else {
        rows.push(ContentFile {
            path: listed.path.clone(),
            origin: Origin::Added,
            sha1: String::new(),
            project_id: Some(project.clone()),
            version_id: None,
            pending: None,
        });
    }
    let dir = kind_folder(listed.kind, &folder.level_name());
    place_version(content, server_id, &folder, &dir, &version, &rows).await
}

/// Place a file uploaded from the App: `.jar` files are mods (plugins on Paper), `.zip` files
/// datapacks. An existing file of that name is replaced.
pub async fn upload(
    content: &Content,
    server_id: &str,
    filename: &str,
    staged: &Path,
) -> Result<(), Error> {
    let server = servers_records::get(&content.db, server_id)
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
        AddonKind::Datapack
    } else if name.ends_with(".jar") {
        if loader == Some(ModLoader::Paper) {
            AddonKind::Plugin
        } else {
            AddonKind::Mod
        }
    } else {
        return Err(Error::Invalid(format!(
            "{filename} is neither a .jar nor a datapack .zip."
        )));
    };
    let folder = Arc::new(Folder::open(&content.paths.server(server_id))?);
    let path = format!("{}/{name}", kind_folder(kind, &folder.level_name()));
    let sha1 = {
        let folder = Arc::clone(&folder);
        let path = path.clone();
        let staged = staged.to_owned();
        tokio::task::spawn_blocking(move || -> std::io::Result<String> {
            let mut reader =
                super::archive::Hashing::new(std::fs::File::open(staged)?);
            folder.write(&path, &mut reader)?;
            Ok(reader.finish())
        })
        .await
        .map_err(std::io::Error::other)??
    };
    // Over a pack file, the row keeps what the pack placed, so the upload counts as the owner's
    // edit and a pack update leaves it alone.
    let pack_file = records::file(&content.db, server_id, &path)
        .await?
        .is_some_and(|row| row.origin == Origin::Pack);
    if !pack_file {
        records::put(
            &content.db,
            server_id,
            &ContentFile {
                path,
                origin: Origin::Added,
                sha1,
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

/// The newest version for each listed file Modrinth knows, by path. Best effort: empty when
/// Modrinth cannot be reached.
pub async fn updates(
    content: &Content,
    server_id: &str,
    listed: &[Listed],
    rows: &[ContentFile],
) -> std::collections::HashMap<String, Version> {
    let Ok(target) = target(content, server_id).await else {
        return std::collections::HashMap::default();
    };
    let by_path: std::collections::HashMap<&str, &ContentFile> =
        rows.iter().map(|row| (row.path.as_str(), row)).collect();
    let mut wanted = Vec::new();
    for item in listed {
        if let Some(row) = by_path.get(item.path.as_str())
            && row.project_id.is_some()
        {
            wanted.push((item, row.sha1.clone(), row.version_id.clone()));
        }
    }
    let mut found = std::collections::HashMap::new();
    for kind in kinds(Some(target.loader)) {
        let of_kind: Vec<_> = wanted
            .iter()
            .filter(|(item, ..)| item.kind == kind)
            .collect();
        let sha1s: Vec<String> =
            of_kind.iter().map(|(_, sha1, _)| sha1.clone()).collect();
        match content
            .modrinth
            .updates(&sha1s, &loaders(&target, kind), &target.game_version)
            .await
        {
            Ok(mut newer) => {
                for (item, sha1, current) in of_kind {
                    if let Some(version) = newer.remove(sha1)
                        && current.as_deref() != Some(version.id.as_str())
                    {
                        found.insert(item.path.clone(), version);
                    }
                }
            }
            Err(error) => {
                warn!(server = %server_id, %error, "could not check for updates")
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_enabled_files_on_disk_provide_a_dependency() {
        let row = |path: &str, project: &str| ContentFile {
            path: path.into(),
            origin: Origin::Added,
            sha1: "x".into(),
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
