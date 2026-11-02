//! Addons: mods, plugins, and datapacks, as files in their folders. A disabled addon is its file
//! with `.disabled` added; its row keeps the enabled path, so a pack update treats a disabled
//! pack file like one the owner removed and leaves it alone.
//!
//! Which version to install and which dependencies come with it is
//! `modrinth_content_management::resolve_content`'s, as in the App: the update channel falls back
//! one step at a time when nothing on it fits, and a dependency it cannot place is reported in
//! the log instead of failing the add.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use bedrock::content::least_stable;
use modrinth_content_management::{
    ContentType, ResolutionPreferences, ResolveContentPlan,
    ResolveContentRequest, SkippedReason, VersionType, resolve_content,
};
use tracing::warn;

use super::{
    Content, Error,
    folder::{self, Folder},
    install::{MAX_FILE, identify},
    modrinth::{self, Version},
    records::{self, ContentFile, Origin, SideToggle},
};
use crate::{
    installs::{
        ModLoader,
        download::{self, FileDownload, Hash},
    },
    servers::records as servers_records,
};

/// Added to an addon's file name to disable it.
pub const DISABLED: &str = ".disabled";

/// The folder addons of `kind` live in. Datapacks are inside the world; adding one on purpose is
/// the one way content goes there. A modpack is not an addon and has none.
pub fn kind_folder(
    kind: ContentType,
    level_name: &str,
) -> Result<String, Error> {
    Ok(match kind {
        ContentType::Mod => "mods".into(),
        ContentType::Plugin => "plugins".into(),
        ContentType::DataPack => format!("{level_name}/datapacks"),
        ContentType::Shader => "shaderpacks".into(),
        ContentType::ResourcePack => "resourcepacks".into(),
        ContentType::ModPack => {
            return Err(Error::Invalid("A modpack is not an addon.".into()));
        }
    })
}

/// The kinds a server with `loader` lists.
pub fn kinds(loader: Option<ModLoader>) -> Vec<ContentType> {
    match loader {
        Some(ModLoader::Paper) => {
            vec![ContentType::Plugin, ContentType::DataPack]
        }
        Some(ModLoader::Vanilla) | None => vec![ContentType::DataPack],
        Some(_) => vec![ContentType::Mod, ContentType::DataPack],
    }
}

/// One addon file on disk.
#[derive(Debug, Clone)]
pub struct Listed {
    /// The enabled path, which its row uses.
    pub path: String,
    pub kind: ContentType,
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
        let Ok(dir) = kind_folder(kind, &level) else {
            continue;
        };
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
            channel: server.update_channel,
        }),
        _ => Err(Error::Invalid(
            "Install a loader on this server first.".into(),
        )),
    }
}

/// The Modrinth loaders content of `kind` is picked by on this server. A modpack's are the
/// server's loader's, as for mods.
fn loaders(target: &Target, kind: ContentType) -> Vec<&'static str> {
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
async fn resolve(
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
pub async fn add(
    content: &Content,
    server_id: &str,
    project_id: &str,
    version_id: Option<&str>,
    kind: Option<ContentType>,
) -> Result<(), Error> {
    let target = target(content, server_id).await?;
    let rows = records::files(&content.db, server_id).await?;
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
            limit: Some(MAX_FILE),
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
    kind: ContentType,
    filename: &str,
) -> Result<(String, String), Error> {
    let name = folder::clean(filename.trim_end_matches(DISABLED))?;
    if name.contains('/') {
        return Err(Error::Invalid(format!("{filename} is not a file name.")));
    }
    let path = format!("{}/{name}", kind_folder(kind, &folder.level_name())?);
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
    kind: ContentType,
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
    kind: ContentType,
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
    kind: ContentType,
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
    kind: ContentType,
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
    // The version installed now: the one recorded, or the one Modrinth knows by its hash.
    let installed = match row.as_ref().and_then(|row| row.version_id.as_deref())
    {
        Some(id) => Some(content.modrinth.version(id).await?),
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
    let dir = kind_folder(listed.kind, &folder.level_name())?;
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

/// The newest version for each listed file Modrinth knows, by path, on the channel it updates
/// on: the least stable of the server's and the one its installed version (in `installed`, by
/// id) is on. Each channel falls back one step at a time, as in the App. Best effort: empty
/// when Modrinth cannot be reached.
pub async fn updates(
    content: &Content,
    server_id: &str,
    listed: &[Listed],
    rows: &[ContentFile],
    installed: &HashMap<String, Version>,
) -> HashMap<String, Version> {
    let Ok(target) = target(content, server_id).await else {
        return HashMap::default();
    };
    let by_path: HashMap<&str, &ContentFile> =
        rows.iter().map(|row| (row.path.as_str(), row)).collect();
    // The files to check, by kind and channel, since each asks Modrinth on its own.
    let mut groups: HashMap<
        (ContentType, VersionType),
        Vec<(&Listed, &ContentFile)>,
    > = HashMap::new();
    for item in listed {
        let Some(row) = by_path
            .get(item.path.as_str())
            .filter(|row| row.project_id.is_some())
        else {
            continue;
        };
        let channel = row
            .version_id
            .as_ref()
            .and_then(|id| installed.get(id))
            .map_or(target.channel, |version| {
                least_stable(target.channel, version.version_type)
            });
        groups
            .entry((item.kind, channel))
            .or_default()
            .push((item, row));
    }
    let mut found = HashMap::new();
    for ((kind, channel), files) in groups {
        let sha1s: Vec<String> =
            files.iter().map(|(_, row)| row.sha1.clone()).collect();
        match content
            .modrinth
            .updates_on_channel(
                &sha1s,
                &loaders(&target, kind),
                &target.game_version,
                channel,
            )
            .await
        {
            Ok(mut newer) => {
                for (item, row) in files {
                    if let Some(version) = newer.remove(&row.sha1)
                        && row.version_id.as_deref()
                            != Some(version.id.as_str())
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

/// The newest version of the Modrinth pack `project_id` a server can update to, when it is not
/// `installed`: one for the server's Minecraft and loader, on the least stable of the server's
/// channel and the installed version's.
pub async fn pack_update(
    content: &Content,
    server_id: &str,
    project_id: &str,
    installed: Option<&Version>,
) -> Result<Option<String>, Error> {
    let target = target(content, server_id).await?;
    let channel = installed.map_or(target.channel, |version| {
        least_stable(target.channel, version.version_type)
    });
    let newest = match resolve(
        content,
        &target,
        project_id,
        None,
        ContentType::ModPack,
        HashSet::new(),
        channel,
    )
    .await
    {
        Ok(plan) => plan.primary.version_id,
        Err(Error::Resolve(
            modrinth_content_management::Error::NoCompatibleVersion(_),
        )) => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(Some(newest).filter(|newest| {
        installed.is_none_or(|installed| installed.id != *newest)
    }))
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
