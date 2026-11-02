//! What restoring a version of a server-first server's share does to the server's addons, by the
//! rule the App's restore follows for an instance (app-lib's `replaced_by_shared_update`): what
//! the share carried at its last push, or carries in the restored version, is replaced by the
//! version, and what the owner added since the last push stays. The pack's own files are left
//! to the pack.

use std::collections::HashSet;

use sharing_client::{
    history::HistoryEntry, rules::CONFIG_FILE_TYPE, versions::ManifestFile,
};

/// An addon on the server, as a restore reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Present {
    /// The share type it is carried as (`server_share::file_type`).
    pub file_type: &'static str,
    pub file_name: String,
    /// Turned off: `.disabled` on disk.
    pub disabled: bool,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    /// The SHA-1 of the file on disk.
    pub sha1: Option<String>,
}

/// What the share carried at its last push or carries in the restored version, which a restore
/// may replace. Everything else on the server was added since the last push.
#[derive(Debug, Default)]
pub struct Shared {
    pub version_ids: HashSet<String>,
    /// The projects of `version_ids`, so a mod updated since the push is replaced too.
    pub project_ids: HashSet<String>,
    /// Carried files by type and name.
    pub files: HashSet<(String, String)>,
}

impl Shared {
    /// The content of `last` and `restored`, without the projects, which only Modrinth knows.
    pub fn of(last: Option<&HistoryEntry>, restored: &HistoryEntry) -> Self {
        let mut shared = Self::default();
        for entry in last.into_iter().chain([restored]) {
            shared
                .version_ids
                .extend(entry.content.modrinth_ids.iter().cloned());
            shared.files.extend(
                entry
                    .manifest()
                    .filter(|file| file.file_type != CONFIG_FILE_TYPE)
                    .map(|file| {
                        (file.file_type.clone(), file.file_name.clone())
                    }),
            );
        }
        shared
    }

    fn carries(&self, addon: &Present) -> bool {
        addon
            .version_id
            .as_ref()
            .is_some_and(|id| self.version_ids.contains(id))
            || addon
                .project_id
                .as_ref()
                .is_some_and(|id| self.project_ids.contains(id))
            || self.files.contains(&(
                addon.file_type.to_owned(),
                addon.file_name.clone(),
            ))
    }
}

/// What a restore does to the server's addons.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct AddonChanges {
    /// Shared addons the version does not have.
    pub remove: Vec<Present>,
    /// Modrinth versions of the version the server does not have.
    pub versions: Vec<String>,
    /// Files of the version the server does not have as they were pushed, each with whether the
    /// copy it replaces is turned off, so it stays off.
    pub files: Vec<(ManifestFile, bool)>,
}

/// How `present`, the server's addons outside its pack, become `restored`.
pub fn addon_changes(
    present: Vec<Present>,
    shared: &Shared,
    restored: &HistoryEntry,
) -> AddonChanges {
    let files = restored
        .manifest()
        .filter(|file| file.file_type != CONFIG_FILE_TYPE)
        .collect::<Vec<_>>();
    let is = |addon: &Present, file: &ManifestFile| {
        addon.file_type == file.file_type && addon.file_name == file.file_name
    };
    let (kept, remove): (Vec<_>, Vec<_>) =
        present.into_iter().partition(|addon| {
            let wanted =
                addon.version_id.as_ref().is_some_and(|id| {
                    restored.content.modrinth_ids.contains(id)
                }) || files.iter().any(|file| is(addon, file));
            wanted || !shared.carries(addon)
        });
    let versions = restored
        .content
        .modrinth_ids
        .iter()
        .filter(|id| {
            !kept
                .iter()
                .any(|addon| addon.version_id.as_ref() == Some(id))
        })
        .cloned()
        .collect();
    let files = files
        .into_iter()
        .filter_map(|file| {
            let current = kept.iter().find(|addon| is(addon, file));
            let same = current
                .and_then(|addon| addon.sha1.as_deref())
                .is_some_and(|sha1| sha1.eq_ignore_ascii_case(&file.sha1));
            (!same).then(|| {
                (file.clone(), current.is_some_and(|addon| addon.disabled))
            })
        })
        .collect();
    AddonChanges {
        remove,
        versions,
        files,
    }
}

#[cfg(test)]
mod tests {
    use sharing_client::{history::HistoryFile, versions::VersionContent};

    use super::*;

    fn entry(
        modrinth_ids: &[&str],
        files: &[(&str, &str, &str)],
    ) -> HistoryEntry {
        HistoryEntry {
            version: 1,
            created: 0,
            content: VersionContent {
                modrinth_ids: modrinth_ids
                    .iter()
                    .map(|id| (*id).into())
                    .collect(),
                modpack_id: None,
                removed_files: Vec::new(),
                game_version: "1.21.1".into(),
                loader: "fabric".into(),
                loader_version: "0.16.0".into(),
            },
            files: files
                .iter()
                .map(|(file_type, file_name, sha1)| HistoryFile {
                    manifest: ManifestFile {
                        file_name: (*file_name).into(),
                        file_type: (*file_type).into(),
                        file_size: 1,
                        sha1: (*sha1).into(),
                        sha256: [0; 32],
                        object: [0; 32],
                        object_size: 17,
                        key: None,
                    },
                    blob: String::new(),
                })
                .collect(),
            diffs: Vec::new(),
        }
    }

    fn modrinth(name: &str, project: &str, version: &str) -> Present {
        Present {
            file_type: "mod",
            file_name: name.into(),
            disabled: false,
            project_id: Some(project.into()),
            version_id: Some(version.into()),
            sha1: Some("x".into()),
        }
    }

    fn file(name: &str, sha1: &str, disabled: bool) -> Present {
        Present {
            file_type: "mod",
            file_name: name.into(),
            disabled,
            project_id: None,
            version_id: None,
            sha1: Some(sha1.into()),
        }
    }

    #[test]
    fn a_restore_replaces_what_was_shared_and_keeps_what_was_added_since() {
        // Pushed last: sodium 2, lithium, and two files of the owner's.
        let last = entry(
            &["sodium-2", "lithium-1"],
            &[("mod", "own.jar", "b"), ("mod", "gone.jar", "c")],
        );
        // Restored: sodium 1 and own.jar as it was then.
        let restored = entry(
            &["sodium-1", "iris-1"],
            &[("mod", "own.jar", "a"), ("config", "config/a.toml", "c")],
        );
        let mut shared = Shared::of(Some(&last), &restored);
        shared.project_ids =
            ["sodium", "lithium", "iris"].map(String::from).into();
        let present = vec![
            modrinth("sodium-2.jar", "sodium", "sodium-2"),
            modrinth("lithium.jar", "lithium", "lithium-1"),
            // Updated since the push: still the share's project.
            modrinth("iris-2.jar", "iris", "iris-2"),
            // Added since the push.
            modrinth("added.jar", "added", "added-1"),
            file("mine.jar", "m", false),
            file("own.jar", "b", true),
            file("gone.jar", "c", false),
        ];

        let changes = addon_changes(present, &shared, &restored);

        let removed = changes
            .remove
            .iter()
            .map(|addon| addon.file_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            removed,
            ["sodium-2.jar", "lithium.jar", "iris-2.jar", "gone.jar"]
        );
        assert_eq!(changes.versions, ["sodium-1", "iris-1"]);
        let files = changes
            .files
            .iter()
            .map(|(file, disabled)| (file.file_name.as_str(), *disabled))
            .collect::<Vec<_>>();
        // Turned off on the server, so it is written back turned off.
        assert_eq!(files, [("own.jar", true)]);
    }

    #[test]
    fn what_the_server_already_has_is_left_alone() {
        let restored = entry(&["sodium-1"], &[("mod", "own.jar", "A")]);
        let shared = Shared::of(None, &restored);
        let present = vec![
            modrinth("sodium.jar", "sodium", "sodium-1"),
            file("own.jar", "a", false),
        ];
        assert_eq!(
            addon_changes(present, &shared, &restored),
            AddonChanges::default()
        );
    }
}
