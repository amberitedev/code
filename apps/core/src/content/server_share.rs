//! What the share of a server-first server carries (a server that follows no instance, so its
//! Core writes its share; see `api::share`). Players get the server's addons that are on for
//! them, whether or not the server runs them: Modrinth versions by id, other files as they are on
//! disk. Plugins run only on a server and are never shared.
//!
//! A server installed from a Modrinth modpack shares that modpack, as the App does, so players
//! also get its client-only files, which never land on the server. Its pack files are then left
//! to the modpack, and the ones the owner removed or turned off for players are listed as removed.

use std::collections::HashMap;

use super::{
    addons::{Listed, Sides},
    records::{ContentFile, Origin, SideToggle},
};
use sharing_client::push::RemovedFile;

use crate::api::content::AddonKind;

/// The pack files of a shared modpack that players do not get: deleted from the server, or on
/// disk but off for players. A file Modrinth knows goes by version; any other mod by its path.
pub fn removed(
    listed: &[Listed],
    rows: &HashMap<String, ContentFile>,
    toggles: &HashMap<String, SideToggle>,
    addon_dirs: &[String],
) -> Vec<RemovedFile> {
    let mut pack_rows = rows
        .values()
        .filter(|row| row.origin == Origin::Pack)
        .collect::<Vec<_>>();
    pack_rows.sort_by(|a, b| a.path.cmp(&b.path));
    let mut removed = Vec::new();
    for row in pack_rows {
        let for_players = match listed.iter().find(|item| item.path == row.path)
        {
            Some(item) => {
                Sides::of(item, toggles.get(&item.path)).for_players()
            }
            // Gone from disk. Only addons count; the pack's other files are not content.
            None => !row.path.rsplit_once('/').is_some_and(|(dir, _)| {
                addon_dirs.iter().any(|addon_dir| addon_dir == dir)
            }),
        };
        if for_players {
            continue;
        }
        let entry = match (&row.version_id, row.path.split_once('/')) {
            (Some(version_id), _) => RemovedFile::Version {
                version_id: version_id.clone(),
            },
            (None, Some(("mods", filename)))
                if !filename.contains('/') && removable(filename) =>
            {
                RemovedFile::Path {
                    parent: "mods".into(),
                    filename: filename.into(),
                }
            }
            _ => continue,
        };
        if !removed.contains(&entry) {
            removed.push(entry);
        }
    }
    removed
}

/// Whether the App takes `filename` as a removed path: only addon archives, as
/// `SharedInstanceRemovedFile::validate` checks. It drops a version listing anything else.
fn removable(filename: &str) -> bool {
    [".jar", ".zip", ".jar.disabled", ".zip.disabled"]
        .iter()
        .any(|suffix| filename.ends_with(suffix))
}

/// One addon as the share carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carried {
    Modrinth {
        version_id: String,
        project_id: String,
    },
    File {
        /// The manifest's name and type: the file name, and `mod` or `datapack`.
        file_name: String,
        file_type: &'static str,
        /// Where its bytes are, relative to the server folder.
        on_disk: String,
    },
}

/// The addons of `listed` that the share carries, in order. With `modpack` the share carries
/// the modpack, so the pack's own files are left to it.
pub fn carried(
    listed: &[Listed],
    rows: &HashMap<String, ContentFile>,
    toggles: &HashMap<String, SideToggle>,
    modpack: bool,
) -> Vec<Carried> {
    listed
        .iter()
        .filter(|item| Sides::of(item, toggles.get(&item.path)).for_players())
        .filter(|item| {
            !modpack
                || rows
                    .get(&item.path)
                    .is_none_or(|row| row.origin != Origin::Pack)
        })
        .filter_map(|item| {
            let file_type = match item.kind {
                AddonKind::Mod => "mod",
                AddonKind::Datapack => "datapack",
                AddonKind::Plugin
                | AddonKind::Shader
                | AddonKind::Resourcepack => return None,
            };
            let row = rows.get(&item.path);
            if let Some((version_id, project_id)) = row.and_then(|row| {
                Some((row.version_id.clone()?, row.project_id.clone()?))
            }) {
                return Some(Carried::Modrinth {
                    version_id,
                    project_id,
                });
            }
            Some(Carried::File {
                file_name: item.filename.clone(),
                file_type,
                on_disk: if item.disabled {
                    format!("{}{}", item.path, super::addons::DISABLED)
                } else {
                    item.path.clone()
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::records::Origin;

    fn listed(path: &str, kind: AddonKind, disabled: bool) -> Listed {
        Listed {
            path: path.into(),
            kind,
            filename: path.rsplit('/').next().unwrap().into(),
            disabled,
            size: 1,
            created: None,
        }
    }

    fn toggle(path: &str, server: bool, player: bool) -> (String, SideToggle) {
        (
            path.into(),
            SideToggle {
                path: path.into(),
                disabled_server: server,
                disabled_player: player,
                unlocked: true,
            },
        )
    }

    #[test]
    fn the_share_carries_what_players_get() {
        let listed = [
            listed("mods/both.jar", AddonKind::Mod, false),
            // Off for the server: `.disabled` on disk, still shared.
            listed("mods/client.jar", AddonKind::Mod, true),
            // Off for players: runs, not shared.
            listed("mods/server.jar", AddonKind::Mod, false),
            // Off for both.
            listed("mods/off.jar", AddonKind::Mod, true),
            // Renamed back by hand after it was turned off for the server: it runs again.
            listed("mods/back.jar", AddonKind::Mod, false),
            listed("mods/modrinth.jar", AddonKind::Mod, false),
            listed("world/datapacks/pack.zip", AddonKind::Datapack, false),
            listed("plugins/plugin.jar", AddonKind::Plugin, false),
        ];
        let toggles = HashMap::from([
            toggle("mods/client.jar", true, false),
            toggle("mods/server.jar", false, true),
            toggle("mods/back.jar", true, false),
        ]);
        let rows = HashMap::from([(
            "mods/modrinth.jar".to_owned(),
            ContentFile {
                path: "mods/modrinth.jar".into(),
                origin: Origin::Added,
                sha1: "x".into(),
                project_id: Some("project".into()),
                version_id: Some("version".into()),
                pending: None,
            },
        )]);

        let file = |file_name: &str, file_type, on_disk: &str| Carried::File {
            file_name: file_name.into(),
            file_type,
            on_disk: on_disk.into(),
        };
        assert_eq!(
            carried(&listed, &rows, &toggles, false),
            [
                file("both.jar", "mod", "mods/both.jar"),
                file("client.jar", "mod", "mods/client.jar.disabled"),
                file("back.jar", "mod", "mods/back.jar"),
                Carried::Modrinth {
                    version_id: "version".into(),
                    project_id: "project".into(),
                },
                file("pack.zip", "datapack", "world/datapacks/pack.zip"),
            ]
        );
    }

    #[test]
    fn a_shared_modpack_carries_only_what_the_owner_changed() {
        let row = |path: &str, origin, version_id: Option<&str>| {
            (
                path.to_owned(),
                ContentFile {
                    path: path.into(),
                    origin,
                    sha1: "x".into(),
                    project_id: version_id.map(|_| "project".into()),
                    version_id: version_id.map(Into::into),
                    pending: None,
                },
            )
        };
        let listed = [
            listed("mods/kept.jar", AddonKind::Mod, false),
            // Off for the server: players still get it from the pack.
            listed("mods/client.jar", AddonKind::Mod, true),
            listed("mods/server.jar", AddonKind::Mod, false),
            listed("mods/added.jar", AddonKind::Mod, false),
        ];
        let toggles = HashMap::from([
            toggle("mods/client.jar", true, false),
            toggle("mods/server.jar", false, true),
        ]);
        let rows = HashMap::from([
            row("mods/kept.jar", Origin::Pack, Some("kept")),
            row("mods/client.jar", Origin::Pack, None),
            row("mods/server.jar", Origin::Pack, Some("server")),
            // Deleted by the owner.
            row("mods/deleted.jar", Origin::Pack, None),
            // A config the pack placed, deleted: not content.
            row("config/a.toml", Origin::Pack, None),
            row("mods/added.jar", Origin::Added, Some("added")),
        ]);

        assert_eq!(
            carried(&listed, &rows, &toggles, true),
            [Carried::Modrinth {
                version_id: "added".into(),
                project_id: "project".into(),
            }]
        );
        assert_eq!(
            removed(&listed, &rows, &toggles, &["mods".into()]),
            [
                RemovedFile::Path {
                    parent: "mods".into(),
                    filename: "deleted.jar".into(),
                },
                RemovedFile::Version {
                    version_id: "server".into(),
                },
            ]
        );
    }

    #[test]
    fn only_addon_archives_are_removed_by_path() {
        let row = |path: &str| {
            (
                path.to_owned(),
                ContentFile {
                    path: path.into(),
                    origin: Origin::Pack,
                    sha1: "x".into(),
                    project_id: None,
                    version_id: None,
                    pending: None,
                },
            )
        };
        let rows = HashMap::from([
            row("mods/gone.jar"),
            row("mods/gone.zip"),
            row("mods/readme.txt"),
            row("mods/gone"),
        ]);
        assert_eq!(
            removed(&[], &rows, &HashMap::new(), &["mods".into()]),
            ["gone.jar", "gone.zip"].map(|filename| RemovedFile::Path {
                parent: "mods".into(),
                filename: filename.into(),
            })
        );
    }
}
