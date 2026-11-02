//! Putting a server-first server back to a version from its share's history (app-lib's
//! `restore_shared_instance_version`). A restore is local: it changes the server and nothing
//! else, and the owner pushes to give the version to anyone. It reads only this Core's history,
//! never the pool.
//!
//! As in the App, what the share carried is replaced and what the owner added since the last
//! push stays (`domain::restore`). Every config of the version is written back, and one the last
//! push shared that the version does not is removed while it is still as pushed. Addons and
//! configs change the way the addon routes change them, whether the server runs or not. A version
//! on another loader, Minecraft version or modpack installs that first, which needs the server
//! stopped like any loader or pack install, and goes on in the background.
//!
//! The pack's own files are left to the pack. A version names the pack files players do not get,
//! but not whether the owner deleted one or only turned it off for players, so a restore cannot
//! tell which to delete.

use std::{collections::HashMap, sync::Arc};

use sharing_client::{
    history::{self, RestorePlan},
    versions::VersionContent,
};

use super::{
    apply_content_install::{kind_of, place_version, target},
    content_mutation::remove_project,
    install_loader::install_loader,
    install_pack::{self as content_install, PackSource},
    share_snapshot::{SCOPE, history_store, snapshot},
};
use crate::{
    installs::ModLoader,
    servers::{Hold, ServerError},
    state::servers::{
        Busy, Content, ContentSourceKind, Error, GameVersionChange, Installing,
        Progress, TrackedFile,
        adapters::{
            filesystem::{
                content_files::{self, DISABLED, addon_path, kind_folder},
                folder::{self, Folder},
            },
            sqlite::{content_rows, server_rows},
        },
        domain::{
            restore::{Present, Shared, addon_changes},
            server_share::{file_type, kind},
        },
    },
};

/// When a restore is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restored {
    /// The server is back to the version.
    Now,
    /// The version's loader or pack installs first, in the background; the install's progress
    /// and result go to the server's console sockets.
    Background,
}

/// Puts server `id`, whose content `busy` holds, back to `version` of its share.
pub async fn restore_share_version(
    content: &Arc<Content>,
    id: &str,
    busy: Busy,
    version: i32,
) -> Result<Restored, Error> {
    let share_id = server_rows::share_id(&content.db, id)
        .await?
        .ok_or_else(|| Error::NotFound("This server is not shared.".into()))?;
    let plan =
        history::restore_plan(&history_store(content), &share_id, version)
            .await?;
    let installed = server_rows::get(&content.db, id)
        .await?
        .and_then(|server| server.loader)
        .and_then(|loader| ModLoader::parse(&loader));
    let current = snapshot(content, id).await?.content;
    let Some(setup) = Setup::of(installed, &current, &plan.entry.content)?
    else {
        apply(content, id, &plan).await?;
        drop(busy);
        return Ok(Restored::Now);
    };
    let hold = content.servers.hold(id).map_err(|error| match error {
        ServerError::AlreadyRunning => Error::Running(
            "Stop the server to restore a version on another loader, Minecraft version or modpack."
                .into(),
        ),
        error => error.into(),
    })?;
    let installing = match setup {
        Setup::Pack { .. } => Installing::Modpack,
        Setup::Loader { .. } => Installing::Loader,
    };
    content.spawn(
        hold,
        busy,
        installing,
        move |content, hold, progress| async move {
            let change = setup.run(&content, &hold, &progress).await?;
            apply(&content, hold.id(), &plan).await?;
            Ok(change)
        },
    );
    Ok(Restored::Background)
}

/// What a version on another loader, Minecraft version or modpack installs before its content.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Setup {
    /// The version's modpack, from Modrinth.
    Pack { version_id: String },
    /// The version's loader and Minecraft. With `unlink` the server first stops following its
    /// pack, whose files stay, as any bare install leaves them.
    Loader {
        loader: ModLoader,
        game_version: String,
        loader_version: Option<String>,
        unlink: bool,
    },
}

impl Setup {
    /// What the server, which runs `installed` and would push `current`, installs to run
    /// `restored`. `None` when it runs it already.
    fn of(
        installed: Option<ModLoader>,
        current: &VersionContent,
        restored: &VersionContent,
    ) -> Result<Option<Self>, Error> {
        let setup = |content: &VersionContent| {
            (
                content.modpack_id.clone(),
                content.game_version.clone(),
                content.loader.clone(),
                content.loader_version.clone(),
            )
        };
        if setup(current) == setup(restored) {
            return Ok(None);
        }
        if let Some(version_id) = &restored.modpack_id
            && restored.modpack_id != current.modpack_id
        {
            return Ok(Some(Self::Pack {
                version_id: version_id.clone(),
            }));
        }
        // A share names Paper as vanilla, which players install as Fabric.
        let loader = match ModLoader::parse(&restored.loader) {
            Some(ModLoader::Vanilla) if installed == Some(ModLoader::Paper) => {
                ModLoader::Paper
            }
            Some(loader) => loader,
            None => {
                return Err(Error::Invalid(format!(
                    "Core does not run {}.",
                    restored.loader
                )));
            }
        };
        Ok(Some(Self::Loader {
            loader,
            game_version: restored.game_version.clone(),
            loader_version: Some(restored.loader_version.clone())
                .filter(|version| !version.is_empty()),
            unlink: restored.modpack_id.is_none()
                && current.modpack_id.is_some(),
        }))
    }

    async fn run(
        self,
        content: &Content,
        hold: &Hold,
        progress: &Progress,
    ) -> Result<Option<GameVersionChange>, Error> {
        match self {
            Self::Pack { version_id } => {
                let project_id =
                    content.modrinth.version(&version_id).await?.project_id;
                content_install::pack(
                    content,
                    hold,
                    progress,
                    PackSource::Modrinth {
                        project_id,
                        version_id,
                    },
                    None,
                    false,
                )
                .await
            }
            Self::Loader {
                loader,
                game_version,
                loader_version,
                unlink,
            } => {
                if unlink {
                    content_rows::unlink(&content.db, hold.id()).await?;
                }
                install_loader(
                    content,
                    hold,
                    progress,
                    loader,
                    &game_version,
                    loader_version.as_deref(),
                    false,
                )
                .await?;
                Ok(None)
            }
        }
    }
}

/// Puts the server's addons and configs back to `plan`'s version.
async fn apply(
    content: &Content,
    id: &str,
    plan: &RestorePlan,
) -> Result<(), Error> {
    let target = target(content, id).await?;
    let folder = Arc::new(Folder::open(&content.paths.server(id))?);
    let rows = content_rows::get_tracked_files(&content.db, id)
        .await?
        .into_iter()
        .map(|row| (row.relative_path.clone(), row))
        .collect::<HashMap<_, _>>();

    // The server's addons outside its pack, hashed off the async workers.
    let present = {
        let folder = Arc::clone(&folder);
        let sha1s = Arc::clone(&content.sha1s);
        let rows = rows.clone();
        tokio::task::spawn_blocking(move || {
            let mut present = Vec::new();
            for item in content_files::scan(&folder, Some(target.loader)) {
                let Some(file_type) = file_type(item.kind) else {
                    continue;
                };
                let row = rows.get(&item.path);
                if row.is_some_and(TrackedFile::by_pack) {
                    continue;
                }
                let on_disk = if item.disabled {
                    format!("{}{DISABLED}", item.path)
                } else {
                    item.path.clone()
                };
                present.push(Present {
                    file_type,
                    file_name: item.filename,
                    disabled: item.disabled,
                    project_id: row.and_then(|row| row.project_id.clone()),
                    version_id: row
                        .filter(|row| row.project_id.is_some())
                        .and_then(|row| row.version_id.clone()),
                    sha1: folder.sha1_cached(&on_disk, &sha1s)?,
                });
            }
            Ok::<_, std::io::Error>(present)
        })
        .await
        .map_err(std::io::Error::other)??
    };
    let mut shared = Shared::of(plan.last.as_ref(), &plan.entry);
    let ids = shared.version_ids.iter().cloned().collect::<Vec<_>>();
    shared.project_ids = content
        .modrinth
        .versions_by_id(&ids)
        .await?
        .into_iter()
        .map(|version| version.project_id)
        .collect();
    let changes = addon_changes(present, &shared, &plan.entry);

    for addon in &changes.remove {
        if let Some(kind) = kind(addon.file_type) {
            remove_project(content, id, kind, &addon.file_name).await?;
        }
    }
    if !changes.versions.is_empty() {
        let rows = content_rows::get_tracked_files(&content.db, id).await?;
        for version_id in &changes.versions {
            let version = content.modrinth.version(version_id).await?;
            let dir =
                kind_folder(kind_of(&version, &target), &folder.level_name())?;
            place_version(content, id, &folder, &dir, &version, &rows).await?;
        }
    }

    // Each file's copy in the history, written at its path.
    let copy = |file_type: &str, file_name: &str| {
        plan.files
            .iter()
            .find(|(file, _)| {
                file.file_type == file_type && file.file_name == file_name
            })
            .map(|(_, copy)| copy.clone())
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "The saved copy of {file_name} is gone."
                ))
            })
    };
    let mut writes = Vec::new();
    let mut tracked = Vec::new();
    for (file, disabled) in &changes.files {
        let Some(kind) = kind(&file.file_type) else {
            continue;
        };
        let path = addon_path(&folder, kind, &file.file_name)?;
        let on_disk = if *disabled {
            format!("{path}{DISABLED}")
        } else {
            path.clone()
        };
        writes.push((on_disk, copy(&file.file_type, &file.file_name)?));
        // Over a pack file, the row keeps what the pack placed, as for an upload.
        if !rows.get(&path).is_some_and(TrackedFile::by_pack) {
            tracked.push(TrackedFile {
                relative_path: path,
                source_kind: ContentSourceKind::Local,
                sha1: file.sha1.to_ascii_lowercase(),
                size: file.file_size,
                project_id: None,
                version_id: None,
                pending: None,
            });
        }
    }
    for file in &plan.configs.write {
        writes.push((
            config_path(&file.file_name)?,
            copy(&file.file_type, &file.file_name)?,
        ));
    }
    let left = plan
        .configs
        .left
        .iter()
        .map(|file| Ok((config_path(&file.file_name)?, file.sha1.clone())))
        .collect::<Result<Vec<_>, Error>>()?;
    {
        let folder = Arc::clone(&folder);
        tokio::task::spawn_blocking(move || {
            for (path, copy) in writes {
                folder.write(&path, &mut std::fs::File::open(copy)?)?;
            }
            // A copy the owner edited since the push stays theirs.
            for (path, sha1) in left {
                if folder
                    .sha1(&path)?
                    .is_some_and(|on_disk| on_disk.eq_ignore_ascii_case(&sha1))
                {
                    folder.remove(&path)?;
                }
            }
            Ok::<_, std::io::Error>(())
        })
        .await
        .map_err(std::io::Error::other)??;
    }
    for row in &tracked {
        content_rows::upsert_tracked_file(&content.db, id, row).await?;
    }
    Ok(())
}

/// A config's path in the server folder, as a push of the server checks it.
fn config_path(file_name: &str) -> Result<String, Error> {
    let path = folder::clean(file_name.trim_start_matches('/'))?;
    if !SCOPE.carries(&path) {
        return Err(Error::Invalid(format!("{path} cannot be restored.")));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(
        modpack_id: Option<&str>,
        game_version: &str,
        loader: &str,
    ) -> VersionContent {
        VersionContent {
            modrinth_ids: Vec::new(),
            modpack_id: modpack_id.map(Into::into),
            removed_files: Vec::new(),
            game_version: game_version.into(),
            loader: loader.into(),
            loader_version: String::new(),
        }
    }

    #[test]
    fn only_another_loader_minecraft_or_pack_installs_first() {
        let current = content(None, "1.21.1", "vanilla");
        assert_eq!(
            Setup::of(Some(ModLoader::Paper), &current, &current).unwrap(),
            None
        );
        // Paper is shared as vanilla, so going back to another Minecraft keeps Paper.
        assert_eq!(
            Setup::of(
                Some(ModLoader::Paper),
                &current,
                &content(None, "1.20.1", "vanilla")
            )
            .unwrap(),
            Some(Setup::Loader {
                loader: ModLoader::Paper,
                game_version: "1.20.1".into(),
                loader_version: None,
                unlink: false,
            })
        );
        let packed = content(Some("pack-2"), "1.21.1", "fabric");
        assert_eq!(
            Setup::of(
                Some(ModLoader::Fabric),
                &packed,
                &content(Some("pack-1"), "1.21.1", "fabric")
            )
            .unwrap(),
            Some(Setup::Pack {
                version_id: "pack-1".into()
            })
        );
        // A version from before the pack leaves it, keeping its files.
        assert!(matches!(
            Setup::of(
                Some(ModLoader::Fabric),
                &packed,
                &content(None, "1.21.1", "fabric")
            )
            .unwrap(),
            Some(Setup::Loader { unlink: true, .. })
        ));
    }

    #[tokio::test]
    async fn a_restore_writes_back_the_versions_files_and_keeps_additions() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::daemon::paths::Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        let db = crate::db::memory().await;
        let servers = crate::servers::Servers::open(paths.clone(), db.clone())
            .await
            .unwrap();
        let id = servers.create("A").await.unwrap().id;
        let backups = crate::backups::Backups::open(
            paths.clone(),
            db.clone(),
            Arc::clone(&servers),
        )
        .await
        .unwrap();
        let content = Content::open(
            paths.clone(),
            db.clone(),
            Arc::clone(&servers),
            backups,
        )
        .await
        .unwrap();
        server_rows::set_installed(
            &db,
            &id,
            "fabric",
            "1.21.1",
            Some("0.16.0"),
        )
        .await
        .unwrap();
        server_rows::set_share_id(&db, &id, Some("share"))
            .await
            .unwrap();
        let server = paths.server(&id);
        let write = |path: &str, text: &str| {
            let path = server.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };

        // Version 1 had own.jar and a.toml; version 2 had b.toml instead.
        let history = history_store(&content);
        let staged = dir.path().join("staged");
        std::fs::create_dir_all(&staged).unwrap();
        for (version, files) in [
            (
                1,
                vec![
                    ("mod", "own.jar", "jar 1"),
                    ("config", "config/a.toml", "a = 1"),
                ],
            ),
            (2, vec![("config", "config/b.toml", "b = 2")]),
        ] {
            let mut external_files = Vec::new();
            let mut sources = Vec::new();
            for (file_type, file_name, text) in files {
                let source = staged
                    .join(format!("{version}-{}", file_name.replace('/', "-")));
                std::fs::write(&source, text).unwrap();
                external_files.push(sharing_client::versions::ManifestFile {
                    file_name: file_name.into(),
                    file_type: file_type.into(),
                    file_size: text.len() as u64,
                    sha1: crate::installs::download::sha1_of(text.as_bytes())
                        .unwrap(),
                    sha256: [0; 32],
                    object: [0; 32],
                    object_size: 0,
                    key: None,
                });
                sources.push(source);
            }
            let pushed = sharing_client::push::Pushed {
                version,
                previous: None,
                external_files,
                paths: sources,
            };
            history::record(
                &history,
                "share",
                &snapshot(&content, &id).await.unwrap().content,
                &pushed,
                Vec::new(),
                None,
            )
            .await
            .unwrap();
        }
        // The server as pushed in version 2, plus a mod added since and an edit to a.toml,
        // which version 2 no longer shared.
        write("config/b.toml", "b = 2");
        write("config/a.toml", "a = edited");
        write("mods/added.jar", "added");

        let busy = content.busy(&id).unwrap();
        let restored =
            restore_share_version(&content, &id, busy, 1).await.unwrap();

        assert_eq!(restored, Restored::Now);
        let read = |path: &str| std::fs::read_to_string(server.join(path)).ok();
        assert_eq!(read("mods/own.jar").as_deref(), Some("jar 1"));
        assert_eq!(read("config/a.toml").as_deref(), Some("a = 1"));
        // Left the share as pushed, so it goes.
        assert_eq!(read("config/b.toml"), None);
        assert_eq!(read("mods/added.jar").as_deref(), Some("added"));
        // The last push stays the newest entry, so the server has changes to push.
        assert_eq!(
            history::latest(&history, "share")
                .await
                .unwrap()
                .unwrap()
                .version,
            2
        );
    }
}
