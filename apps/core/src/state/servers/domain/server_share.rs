//! What the share of a server-first server carries (a server that follows no instance, so its
//! Core writes its share; see `api::share`). Players get the server's addons that are on for
//! them, whether or not the server runs them: Modrinth versions by id, other files as they are on
//! disk. Plugins run only on a server and are never shared.
//!
//! A server installed from a Modrinth modpack shares that modpack, as the App does, so players
//! also get its client-only files, which never land on the server. Its pack files are then left
//! to the modpack, and the ones the owner removed or turned off for players are listed as removed.

use std::collections::HashMap;

use crate::state::servers::{EntrySides, TrackedFile};
use crate::state::servers::{
    adapters::filesystem::content_files::{DISABLED, Listed},
    domain::sides::Sides,
};
use sharing_client::push::RemovedFile;

use modrinth_content_management::ContentType;

/// The pack files of a shared modpack that players do not get: deleted from the server, or on
/// disk but off for players. A file Modrinth knows goes by version; any other mod by its path.
pub fn removed(
    listed: &[Listed],
    rows: &HashMap<String, TrackedFile>,
    entry_sides: &HashMap<String, EntrySides>,
    addon_dirs: &[String],
) -> Vec<RemovedFile> {
    let mut pack_rows = rows
        .values()
        .filter(|row| row.by_pack())
        .collect::<Vec<_>>();
    pack_rows.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let mut removed = Vec::new();
    for row in pack_rows {
        let for_players =
            match listed.iter().find(|item| item.path == row.relative_path) {
                Some(item) => {
                    Sides::of(item, entry_sides.get(&item.path)).for_players()
                }
                // Gone from disk. Only addons count; the pack's other files are not content.
                None => !row.relative_path.rsplit_once('/').is_some_and(
                    |(dir, _)| {
                        addon_dirs.iter().any(|addon_dir| addon_dir == dir)
                    },
                ),
            };
        if for_players {
            continue;
        }
        let entry = match (&row.version_id, row.relative_path.split_once('/')) {
            (Some(version_id), _) => RemovedFile::Version {
                version_id: version_id.clone(),
            },
            (None, Some(("mods", filename))) if removable(filename) => {
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

/// Whether the App takes `filename` as a removed path: one plain file name of an addon archive,
/// as `SharedInstanceRemovedFile::validate` checks. It drops a version listing anything else.
/// The backend's file names also refuse control characters.
fn removable(filename: &str) -> bool {
    path_util::is_safe_file_name(filename)
        && !filename.contains(char::is_control)
        && [".jar", ".zip", ".jar.disabled", ".zip.disabled"]
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
    rows: &HashMap<String, TrackedFile>,
    entry_sides: &HashMap<String, EntrySides>,
    modpack: bool,
) -> Vec<Carried> {
    listed
        .iter()
        .filter(|item| {
            Sides::of(item, entry_sides.get(&item.path)).for_players()
        })
        .filter(|item| {
            !modpack || rows.get(&item.path).is_none_or(|row| !row.by_pack())
        })
        .filter_map(|item| {
            let file_type = match item.kind {
                ContentType::Mod => "mod",
                ContentType::DataPack => "datapack",
                ContentType::Plugin
                | ContentType::Shader
                | ContentType::ResourcePack
                | ContentType::ModPack => return None,
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
                    format!("{}{}", item.path, DISABLED)
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
    use bedrock::state::{ContentRequirement, ContentSourceKind};

    fn listed(path: &str, kind: ContentType, disabled: bool) -> Listed {
        Listed {
            path: path.into(),
            kind,
            filename: path.rsplit('/').next().unwrap().into(),
            disabled,
            size: 1,
            created: None,
        }
    }

    /// A side turned off is unsupported there.
    fn side(off: bool) -> ContentRequirement {
        if off {
            ContentRequirement::Unsupported
        } else {
            ContentRequirement::Required
        }
    }

    fn sides(path: &str, server: bool, player: bool) -> (String, EntrySides) {
        (
            path.into(),
            EntrySides {
                enabled: true,
                server_requirement: side(server),
                client_requirement: side(player),
                unlocked: true,
            },
        )
    }

    #[test]
    fn the_share_carries_what_players_get() {
        let listed = [
            listed("mods/both.jar", ContentType::Mod, false),
            // Off for the server: `.disabled` on disk, still shared.
            listed("mods/client.jar", ContentType::Mod, true),
            // Off for players: runs, not shared.
            listed("mods/server.jar", ContentType::Mod, false),
            // Off for both.
            listed("mods/off.jar", ContentType::Mod, true),
            // Renamed back by hand after it was turned off for the server: it runs again.
            listed("mods/back.jar", ContentType::Mod, false),
            listed("mods/modrinth.jar", ContentType::Mod, false),
            listed("world/datapacks/pack.zip", ContentType::DataPack, false),
            listed("plugins/plugin.jar", ContentType::Plugin, false),
        ];
        let entry_sides = HashMap::from([
            sides("mods/client.jar", true, false),
            sides("mods/server.jar", false, true),
            sides("mods/back.jar", true, false),
        ]);
        let rows = HashMap::from([(
            "mods/modrinth.jar".to_owned(),
            TrackedFile {
                relative_path: "mods/modrinth.jar".into(),
                source_kind: ContentSourceKind::Local,
                sha1: "x".into(),
                size: 1,
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
            carried(&listed, &rows, &entry_sides, false),
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
        let row = |path: &str, source_kind, version_id: Option<&str>| {
            (
                path.to_owned(),
                TrackedFile {
                    relative_path: path.into(),
                    source_kind,
                    sha1: "x".into(),
                    size: 1,
                    project_id: version_id.map(|_| "project".into()),
                    version_id: version_id.map(Into::into),
                    pending: None,
                },
            )
        };
        let listed = [
            listed("mods/kept.jar", ContentType::Mod, false),
            // Off for the server: players still get it from the pack.
            listed("mods/client.jar", ContentType::Mod, true),
            listed("mods/server.jar", ContentType::Mod, false),
            listed("mods/added.jar", ContentType::Mod, false),
        ];
        let entry_sides = HashMap::from([
            sides("mods/client.jar", true, false),
            sides("mods/server.jar", false, true),
        ]);
        let rows = HashMap::from([
            row(
                "mods/kept.jar",
                ContentSourceKind::ModrinthModpack,
                Some("kept"),
            ),
            row("mods/client.jar", ContentSourceKind::ModrinthModpack, None),
            row(
                "mods/server.jar",
                ContentSourceKind::ModrinthModpack,
                Some("server"),
            ),
            // Deleted by the owner.
            row("mods/deleted.jar", ContentSourceKind::ModrinthModpack, None),
            // A config the pack placed, deleted: not content.
            row("config/a.toml", ContentSourceKind::ModrinthModpack, None),
            row("mods/added.jar", ContentSourceKind::Local, Some("added")),
        ]);

        assert_eq!(
            carried(&listed, &rows, &entry_sides, true),
            [Carried::Modrinth {
                version_id: "added".into(),
                project_id: "project".into(),
            }]
        );
        assert_eq!(
            removed(&listed, &rows, &entry_sides, &["mods".into()]),
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
                TrackedFile {
                    relative_path: path.into(),
                    source_kind: ContentSourceKind::ModrinthModpack,
                    sha1: "x".into(),
                    size: 1,
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

    #[test]
    fn a_removed_path_is_one_safe_archive_name() {
        for name in ["a.jar", "a.zip", "a.jar.disabled", "CONFIG.jar"] {
            assert!(removable(name), "{name}");
        }
        for name in [
            "a\\b.jar",
            "a/b.jar",
            "..jar.disabled/x.jar",
            "a\u{1}.jar",
            "con.jar",
            "C:a.jar",
            "readme.txt",
        ] {
            assert!(!removable(name), "{name}");
        }
    }
}
