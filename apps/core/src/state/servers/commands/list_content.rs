//! A server's content list, and the pack it follows as the modpack card shows it.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use modrinth_content_management::{ContentType, DependencyType};

use engine::sandbox;

use super::check_content_updates::{
    check_content_updates, check_modpack_update,
};
use crate::{
    installs::ModLoader,
    state::servers::{
        Addon, AddonStatus, AddonVersion, Content, Error, Installing,
        LinkedPackState, ModpackFields, ModpackSpec, Pack, SkippedFile,
        TrackedFile,
        adapters::{
            filesystem::content_files::{DISABLED, Listed},
            network::modrinth,
            sqlite::content_rows,
        },
        domain::sides::Sides,
        modrinth_pack, project_type_of,
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
                    environment: version.environment,
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
                environment: version.environment,
            }),
            owner: None,
            icon_url: project.and_then(|project| project.icon_url.clone()),
            manifest: None,
            installed: Some(false),
        })
    }));
    mark_client_dependencies(&mut addons, &versions);
    Ok(addons)
}

/// Upstream's client-only dependency warnings. `pack_client_depends`: the server runs the addon
/// and it requires a client-only project. `pack_client_retained`: the server runs a client-only
/// addon because another addon it runs requires it. Client only means a pack file the server
/// skips, or a version Modrinth says players alone run.
fn mark_client_dependencies(
    addons: &mut [Addon],
    versions: &HashMap<String, modrinth::Version>,
) {
    let runs_on_server = |addon: &Addon| {
        !addon.disabled
            && !addon.disabled_server
            && addon.installed != Some(false)
    };
    let required = |addon: &Addon| -> Vec<String> {
        addon
            .version
            .as_ref()
            .and_then(|version| versions.get(&version.id))
            .map(|version| {
                version
                    .dependencies
                    .iter()
                    .filter(|dependency| {
                        dependency.dependency_type == DependencyType::Required
                    })
                    .filter_map(|dependency| dependency.project_id.clone())
                    .collect()
            })
            .unwrap_or_default()
    };
    let client_only: HashSet<String> = addons
        .iter()
        .filter(|addon| {
            addon.installed == Some(false)
                || !bedrock::content::server_takes(
                    None,
                    addon
                        .version
                        .as_ref()
                        .and_then(|version| version.environment),
                    None,
                )
        })
        .filter_map(|addon| addon.project_id.clone())
        .collect();
    let required_on_server: HashSet<String> = addons
        .iter()
        .filter(|addon| runs_on_server(addon))
        .flat_map(&required)
        .collect();
    for addon in addons.iter_mut().filter(|addon| runs_on_server(addon)) {
        let own = addon.project_id.as_deref();
        addon.pack_client_depends = required(addon).iter().any(|project| {
            Some(project.as_str()) != own && client_only.contains(project)
        });
        addon.pack_client_retained = own.is_some_and(|own| {
            client_only.contains(own) && required_on_server.contains(own)
        });
    }
}

/// The jar on disk for `listed`, opened without following a link.
pub fn open_jar(
    folder: &sandbox::Folder,
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
) -> HashMap<String, engine::server_jar::Manifest> {
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
        let Ok(folder) = sandbox::Folder::open(&root) else {
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
    let (project, version, has_update) = match modrinth_pack(&pack.link) {
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
    let spec = match modrinth_pack(&pack.link) {
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
            .share
            .as_ref()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn addon(project: &str, installed: Option<bool>) -> Addon {
        Addon {
            id: format!("mods/{project}.jar"),
            filename: format!("{project}.jar"),
            filesize: 0,
            btime: None,
            disabled: false,
            disabled_server: installed == Some(false),
            disabled_player: false,
            side_toggle_unlocked: false,
            kind: ContentType::Mod,
            from_modpack: true,
            status: AddonStatus::Installed,
            pack_client_retained: false,
            pack_client_depends: false,
            needs_side_type_review: false,
            has_update: None,
            name: None,
            project_id: Some(project.to_owned()),
            version: Some(AddonVersion {
                id: format!("{project}-v"),
                name: None,
                environment: None,
            }),
            owner: None,
            icon_url: None,
            manifest: None,
            installed,
        }
    }

    fn version(
        project: &str,
        environment: &str,
        requires: &[&str],
    ) -> modrinth::Version {
        let dependencies: Vec<_> = requires
            .iter()
            .map(|dependency| {
                serde_json::json!({
                    "project_id": dependency,
                    "dependency_type": "required",
                })
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "id": format!("{project}-v"),
            "project_id": project,
            "version_number": "1",
            "dependencies": dependencies,
            "date_published": null,
            "environment": environment,
        }))
        .unwrap()
    }

    #[test]
    fn server_addons_that_require_client_only_projects_are_marked() {
        // `uses-skipped` requires a pack file the server skips; `uses-placed` requires a client-only
        // mod the server runs anyway.
        let mut addons = vec![
            addon("uses-skipped", None),
            addon("skipped", Some(false)),
            addon("uses-placed", None),
            addon("placed", None),
            addon("plain", None),
        ];
        let versions: HashMap<String, modrinth::Version> = [
            version("uses-skipped", "client_and_server", &["skipped"]),
            version("skipped", "client_only", &[]),
            version("uses-placed", "client_and_server", &["placed"]),
            version("placed", "client_only", &[]),
            version("plain", "client_and_server", &[]),
        ]
        .into_iter()
        .map(|version| (version.id.clone(), version))
        .collect();
        for addon in &mut addons {
            addon.version.as_mut().unwrap().environment =
                versions[&addon.version.as_ref().unwrap().id].environment;
        }
        mark_client_dependencies(&mut addons, &versions);
        let flags: Vec<(bool, bool)> = addons
            .iter()
            .map(|addon| {
                (addon.pack_client_depends, addon.pack_client_retained)
            })
            .collect();
        assert_eq!(
            flags,
            [
                (true, false),
                (false, false),
                (true, false),
                (false, true),
                (false, false)
            ]
        );
    }
}
