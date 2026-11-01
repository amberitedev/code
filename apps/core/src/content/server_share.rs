//! What the share of a server-first server carries (a server that follows no instance, so its
//! Core writes its share; see `api::share`). Players get the server's addons that are on for
//! them, whether or not the server runs them: Modrinth versions by id, other files as they are on
//! disk. Plugins run only on a server and are never shared.

use std::collections::HashMap;

use super::{
    addons::{Listed, Sides},
    records::{ContentFile, SideToggle},
};
use crate::api::content::AddonKind;

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

/// The addons of `listed` that the share carries, in order.
pub fn carried(
    listed: &[Listed],
    rows: &HashMap<String, ContentFile>,
    toggles: &HashMap<String, SideToggle>,
) -> Vec<Carried> {
    listed
        .iter()
        .filter(|item| Sides::of(item, toggles.get(&item.path)).for_players())
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
            carried(&listed, &rows, &toggles),
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
}
