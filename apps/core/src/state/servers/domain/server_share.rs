//! What the share of a server-first server carries (a server that follows no instance, so its
//! Core writes its share; see `api::share`). Players get the server's addons that are on for
//! them, whether or not the server runs them: Modrinth versions by id, other files as they are on
//! disk. Plugins run only on a server and are never shared.
//!
//! A server installed from a Modrinth modpack shares that modpack, as the App does, so players
//! also get its client-only files, which never land on the server. Its pack files are then left
//! to the modpack, and the ones the owner removed or turned off for players are listed as removed.
//! This file reads the server's rows into what `sharing_client::push` takes; which rows go by
//! Modrinth version, which are carried and how removed files are named is decided there.

use std::collections::HashMap;

use crate::state::servers::{EntrySides, TrackedFile};
use crate::state::servers::{
    adapters::filesystem::content_files::{DISABLED, Listed},
    domain::sides::Sides,
};
use sharing_client::install::ModpackFile;
use sharing_client::push::ContentRow;
use sharing_client::versions::RemovedFile;

use modrinth_content_management::ContentType;

/// The pack files of a shared modpack that players do not get: deleted from the server, or on
/// disk but off for players. A file Modrinth knows goes by version; any other addon by its path.
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
    let dropped = pack_rows
        .into_iter()
        .filter(|row| {
            match listed.iter().find(|item| item.path == row.relative_path) {
                Some(item) => {
                    !Sides::of(item, entry_sides.get(&item.path)).for_players()
                }
                // Gone from disk. Only addons count; the pack's other files are not content.
                None => row.relative_path.rsplit_once('/').is_some_and(
                    |(dir, _)| {
                        addon_dirs.iter().any(|addon_dir| addon_dir == dir)
                    },
                ),
            }
        })
        .map(pack_file)
        .collect::<Vec<_>>();
    sharing_client::push::removed_files(&dropped)
}

/// A file the server's pack placed, as the share names a modpack's files.
pub fn pack_file(row: &TrackedFile) -> ModpackFile {
    ModpackFile {
        path: row.relative_path.clone(),
        sha1: Some(row.sha1.clone()),
        version_id: row.version_id.clone(),
        alias: None,
        source: (),
    }
}

/// The manifest type a server's share carries addons of `kind` as. Plugins run only on a server,
/// and a server has no shaders or resource packs of its own.
pub fn file_type(kind: ContentType) -> Option<&'static str> {
    match kind {
        ContentType::Mod => Some("mod"),
        ContentType::DataPack => Some("datapack"),
        ContentType::Plugin
        | ContentType::Shader
        | ContentType::ResourcePack
        | ContentType::ModPack => None,
    }
}

/// The kind of addon a server's share carries as `file_type`, the other way from [`file_type`].
pub fn kind(file_type: &str) -> Option<ContentType> {
    match file_type {
        "mod" => Some(ContentType::Mod),
        "datapack" => Some(ContentType::DataPack),
        _ => None,
    }
}

/// The addons of `listed` that players get, in order, as a push reads them. With `modpack` the
/// share carries the modpack, so the pack's own files are marked as the modpack's.
pub fn content_rows(
    listed: &[Listed],
    rows: &HashMap<String, TrackedFile>,
    entry_sides: &HashMap<String, EntrySides>,
    modpack: bool,
) -> Vec<ContentRow> {
    listed
        .iter()
        .filter(|item| {
            Sides::of(item, entry_sides.get(&item.path)).for_players()
        })
        .filter_map(|item| {
            let file_type = file_type(item.kind)?;
            let row = rows.get(&item.path);
            Some(ContentRow {
                path: if item.disabled {
                    format!("{}{}", item.path, DISABLED)
                } else {
                    item.path.clone()
                },
                file_name: item.filename.clone(),
                file_type: file_type.to_owned(),
                version_id: row
                    .filter(|row| row.project_id.is_some())
                    .and_then(|row| row.version_id.clone()),
                from_modpack: modpack && row.is_some_and(TrackedFile::by_pack),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock::state::{ContentRequirement, ContentSourceKind};

    /// What a push sends of the server: its Modrinth versions, and each carried file as its
    /// manifest name, type and path on disk.
    fn pushed(
        listed: &[Listed],
        rows: &HashMap<String, TrackedFile>,
        entry_sides: &HashMap<String, EntrySides>,
        modpack: bool,
    ) -> (Vec<String>, Vec<(String, String, String)>) {
        let pack = if modpack {
            rows.values()
                .filter(|row| row.by_pack())
                .map(pack_file)
                .collect()
        } else {
            Vec::new()
        };
        let content = sharing_client::push::read_content(
            content_rows(listed, rows, entry_sides, modpack),
            &pack,
        );
        let carried = content
            .carried
            .into_iter()
            .map(|row| (row.file_name, row.file_type, row.path))
            .collect();
        (content.modrinth_ids, carried)
    }

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
                server_override: false,
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

        let (modrinth_ids, carried) =
            pushed(&listed, &rows, &entry_sides, false);
        assert_eq!(modrinth_ids, ["version"]);
        assert_eq!(
            carried,
            [
                ("both.jar", "mod", "mods/both.jar"),
                ("client.jar", "mod", "mods/client.jar.disabled"),
                ("back.jar", "mod", "mods/back.jar"),
                ("pack.zip", "datapack", "world/datapacks/pack.zip"),
            ]
            .map(|(name, kind, path)| (
                name.into(),
                kind.into(),
                path.into()
            ))
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

        let (modrinth_ids, carried) =
            pushed(&listed, &rows, &entry_sides, true);
        assert_eq!(modrinth_ids, ["added"]);
        assert!(carried.is_empty());
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
        let removable = |name: &str| RemovedFile::path("mods", name).is_some();
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
