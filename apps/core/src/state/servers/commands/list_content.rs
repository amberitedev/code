//! A server's content list, and the pack it follows as the modpack card shows it.

use std::{collections::HashMap, sync::Arc};

use modrinth_content_management::ContentType;

use super::check_content_updates::{
    check_content_updates, check_modpack_update,
};
use crate::{
    api::content::{
        Addon, AddonStatus, AddonVersion, Installing, LinkedPackState,
        ModpackFields, ModpackSpec,
    },
    installs::ModLoader,
    state::servers::{
        Content, Error, Pack, SkippedFile, TrackedFile,
        adapters::{
            filesystem::{
                content_files::{DISABLED, Listed},
                folder,
            },
            network::modrinth,
            sqlite::content_rows,
        },
        domain::sides::Sides,
        project_type_of,
    },
};

/// The content list of a server: `listed`, the addon files on its disk, and `skipped`, the pack's
/// client-only files, with what Modrinth knows about each. `updates` looks for newer versions.
pub async fn list_content(
    content: &Arc<Content>,
    id: &str,
    loader: Option<ModLoader>,
    listed: Vec<Listed>,
    skipped: Vec<SkippedFile>,
    updates: bool,
) -> Result<Vec<Addon>, Error> {
    let mut manifests = manifests(content, id, loader, &listed).await;
    let mut rows = HashMap::new();
    let mut entry_sides = HashMap::new();
    for (row, sides) in
        content_rows::get_tracked_files_with_sides(&content.db, id).await?
    {
        entry_sides.insert(row.relative_path.clone(), sides);
        rows.insert(row.relative_path.clone(), row);
    }
    let mut project_ids: Vec<String> = rows
        .values()
        .filter_map(|row| row.project_id.clone())
        .chain(skipped.iter().filter_map(|file| file.project_id.clone()))
        .collect();
    project_ids.sort();
    project_ids.dedup();
    let version_ids: Vec<String> = rows
        .values()
        .filter_map(|row| row.version_id.clone())
        .chain(skipped.iter().filter_map(|file| file.version_id.clone()))
        .collect();
    let modrinth = &content.modrinth;
    let (projects, versions) = tokio::join!(
        modrinth.projects(&project_ids),
        modrinth.versions_by_id(&version_ids)
    );
    let projects: HashMap<String, modrinth::Project> = projects
        .unwrap_or_default()
        .into_iter()
        .map(|project| (project.id.clone(), project))
        .collect();
    let versions: HashMap<String, modrinth::Version> = versions
        .unwrap_or_default()
        .into_iter()
        .map(|version| (version.id.clone(), version))
        .collect();
    let newer = if updates {
        let row_list: Vec<TrackedFile> = rows.values().cloned().collect();
        check_content_updates(content, id, &listed, &row_list, &versions).await
    } else {
        HashMap::new()
    };
    let mut addons: Vec<Addon> = listed
        .into_iter()
        .map(|item| {
            let row = rows.get(&item.path);
            let project = row
                .and_then(|row| row.project_id.as_ref())
                .and_then(|project| projects.get(project));
            let version = row
                .and_then(|row| row.version_id.as_ref())
                .and_then(|version| versions.get(version));
            let recorded = entry_sides.get(&item.path);
            let sides = Sides::of(&item, recorded);
            Addon {
                id: item.path.clone(),
                filename: item.filename,
                filesize: item.size,
                btime: item.created,
                disabled: sides.disabled,
                disabled_server: sides.disabled_server,
                disabled_player: sides.disabled_player,
                side_toggle_unlocked: recorded
                    .is_some_and(|recorded| recorded.unlocked),
                kind: item.kind,
                from_modpack: row.is_some_and(TrackedFile::by_pack),
                status: AddonStatus::Installed,
                pack_client_retained: false,
                pack_client_depends: false,
                needs_side_type_review: false,
                has_update: newer
                    .get(&item.path)
                    .map(|version| version.id.clone()),
                name: project.map(|project| project.title.clone()),
                project_id: row.and_then(|row| row.project_id.clone()),
                version: version.map(|version| AddonVersion {
                    id: version.id.clone(),
                    name: Some(version.version_number.clone()),
                    environment: None,
                }),
                owner: None,
                icon_url: project.and_then(|project| project.icon_url.clone()),
                manifest: manifests.remove(&item.path),
                installed: None,
            }
        })
        .collect();
    // The pack's client-only files: listed, off for the server, never on its disk.
    addons.extend(skipped.into_iter().filter_map(|file| {
        let kind = project_type_of(&file.relative_path)?;
        let project = file
            .project_id
            .as_ref()
            .and_then(|project| projects.get(project));
        let version = file
            .version_id
            .as_ref()
            .and_then(|version| versions.get(version));
        Some(Addon {
            filename: file
                .relative_path
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned(),
            id: file.relative_path,
            filesize: file.size,
            btime: None,
            disabled: false,
            disabled_server: true,
            disabled_player: false,
            side_toggle_unlocked: false,
            kind,
            from_modpack: true,
            status: AddonStatus::Installed,
            pack_client_retained: false,
            pack_client_depends: false,
            needs_side_type_review: false,
            has_update: None,
            name: project.map(|project| project.title.clone()),
            project_id: file.project_id,
            version: version.map(|version| AddonVersion {
                id: version.id.clone(),
                name: Some(version.version_number.clone()),
                environment: None,
            }),
            owner: None,
            icon_url: project.and_then(|project| project.icon_url.clone()),
            manifest: None,
            installed: Some(false),
        })
    }));
    Ok(addons)
}

/// The jar on disk for `listed`, opened without following a link.
pub fn open_jar(
    folder: &folder::Folder,
    listed: &Listed,
) -> Option<std::fs::File> {
    let on_disk = if listed.disabled {
        format!("{}{}", listed.path, DISABLED)
    } else {
        listed.path.clone()
    };
    Some(folder.open_file(&on_disk).ok()??.into_std())
}

/// What each mod and plugin jar in `listed` says about itself, by path. Best effort: a jar that
/// cannot be read has none.
pub async fn manifests(
    content: &Arc<Content>,
    id: &str,
    loader: Option<ModLoader>,
    listed: &[Listed],
) -> HashMap<String, bedrock::upstream::jar::Manifest> {
    let jars: Vec<Listed> = listed
        .iter()
        .filter(|listed| {
            matches!(listed.kind, ContentType::Mod | ContentType::Plugin)
        })
        .cloned()
        .collect();
    if jars.is_empty() {
        return HashMap::new();
    }
    let content = Arc::clone(content);
    let root = content.paths.server(id);
    let id = id.to_owned();
    tokio::task::spawn_blocking(move || {
        let Ok(folder) = folder::Folder::open(&root) else {
            return HashMap::new();
        };
        jars.into_iter()
            .filter_map(|listed| {
                let file = open_jar(&folder, &listed)?;
                let key = format!("{id}/{}", listed.path);
                let inspection = content.manifests.get(
                    &key,
                    file,
                    loader.and_then(ModLoader::jar_platform),
                )?;
                Some((listed.path, inspection.manifest))
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// The pack a server follows, as the modpack card shows it.
pub async fn get_linked_modpack_info(
    content: &Content,
    id: &str,
    check: bool,
) -> Result<Option<ModpackFields>, Error> {
    let Some(pack) = content_rows::get_pack(&content.db, id).await? else {
        return Ok(None);
    };
    let modrinth = &content.modrinth;
    let (project, version, has_update) = match pack.link.modrinth() {
        Some((project_id, version_id)) => {
            let (project, version) = tokio::join!(
                modrinth.project(project_id),
                modrinth.version(version_id)
            );
            let version = version.ok();
            // Without the installed version there is nothing to compare, as in the App.
            let has_update = match &version {
                Some(installed) if check => {
                    check_modpack_update(content, id, project_id, installed)
                        .await
                        .ok()
                        .flatten()
                }
                _ => None,
            };
            (project.ok(), version, has_update)
        }
        _ => (None, None, None),
    };
    let linked_state = linked_pack_state(content, id, &pack).await?;
    let spec = match pack.link.modrinth() {
        Some((project_id, version_id)) => ModpackSpec::Modrinth {
            project_id: project_id.to_owned(),
            version_id: version_id.to_owned(),
        },
        _ => ModpackSpec::LocalFile {
            filename: format!("{}.mrpack", pack.set.name),
            name: pack.set.name.clone(),
            description: pack.summary.clone(),
        },
    };
    Ok(Some(ModpackFields {
        spec,
        has_update,
        title: Some(
            project
                .as_ref()
                .map_or(pack.set.name.clone(), |project| project.title.clone()),
        ),
        description: project
            .as_ref()
            .and_then(|project| project.description.clone())
            .or(pack.summary.clone()),
        icon_url: project
            .as_ref()
            .and_then(|project| project.icon_url.clone()),
        owner: None,
        version_number: Some(
            version
                .as_ref()
                .map_or(pack.version_number.clone(), |version| {
                    version.version_number.clone()
                }),
        ),
        date_published: version
            .as_ref()
            .and_then(|version| version.date_published),
        downloads: project.as_ref().and_then(|project| project.downloads),
        followers: project.as_ref().and_then(|project| project.followers),
        instance_id: pack
            .link
            .shared_instance()
            .map(|shared| shared.instance_id.clone()),
        state: Some(linked_state),
        error: pack.error,
    }))
}

/// Where a server is with the pack it follows: applying it, failed, waiting for the server to
/// stop to apply a newer push, or up to date.
pub async fn linked_pack_state(
    content: &Content,
    id: &str,
    pack: &Pack,
) -> Result<LinkedPackState, Error> {
    Ok(
        if content.activity(id).installing == Some(Installing::Modpack) {
            LinkedPackState::Applying
        } else if pack.error.is_some() {
            LinkedPackState::Failed
        } else if content_rows::get_pending(&content.db, id).await?.is_some() {
            LinkedPackState::Pending
        } else {
            LinkedPackState::UpToDate
        },
    )
}
