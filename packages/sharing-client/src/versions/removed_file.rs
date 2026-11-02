use serde::{Deserialize, Serialize};

/// A file of a version's modpack that members do not get: by its Modrinth version, or, for a
/// file Modrinth does not know, by its folder and file name. The App lists the modpack files the
/// owner removed or turned off; a Core lists those a server-first server's owner removed or
/// turned off for players.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemovedFile {
    Version { version_id: String },
    Path { parent: String, filename: String },
}

/// A removed file the App and the backend would refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid removed modpack file")]
pub struct InvalidRemovedFile;

/// The folders a removed path may name.
const PARENTS: [&str; 5] = [
    "mods",
    "plugins",
    "datapacks",
    "shaderpacks",
    "resourcepacks",
];
const SUFFIXES: [&str; 4] = [".jar", ".zip", ".jar.disabled", ".zip.disabled"];

impl RemovedFile {
    /// `parent/filename` as a removed path, or `None` when it is not one: one plain file name
    /// of an addon archive in an addon folder.
    pub fn path(parent: &str, filename: &str) -> Option<Self> {
        let removed = Self::Path {
            parent: parent.to_owned(),
            filename: filename.to_owned(),
        };
        removed.validate().ok().map(|()| removed)
    }

    /// Whether this names the modpack file at `path`, with its Modrinth `version_id` when
    /// Modrinth knows it. A file turned off (`.disabled`) is the same file as when it is on.
    pub fn matches(&self, version_id: Option<&str>, path: &str) -> bool {
        match self {
            Self::Version {
                version_id: removed,
            } => version_id == Some(removed.as_str()),
            Self::Path { parent, filename } => {
                fn enabled(name: &str) -> &str {
                    name.strip_suffix(".disabled").unwrap_or(name)
                }
                enabled(path) == format!("{parent}/{}", enabled(filename))
            }
        }
    }

    /// Checks a removed path as the backend does: an addon folder, and one plain file name of
    /// an addon archive without control characters.
    pub fn validate(&self) -> Result<(), InvalidRemovedFile> {
        let Self::Path { parent, filename } = self else {
            return Ok(());
        };
        let valid = PARENTS.contains(&parent.as_str())
            && path_util::is_safe_file_name(filename)
            && !filename.contains(char::is_control)
            && SUFFIXES.iter().any(|suffix| filename.ends_with(suffix));
        valid.then_some(()).ok_or(InvalidRemovedFile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_removed_modpack_file_matches_enabled_or_not() {
        let removed = RemovedFile::path("mods", "a.jar.disabled").unwrap();
        assert!(removed.matches(None, "mods/a.jar"));
        assert!(removed.matches(None, "mods/a.jar.disabled"));
        assert!(!removed.matches(None, "mods/b.jar"));
        let removed = RemovedFile::Version {
            version_id: "v".into(),
        };
        assert!(removed.matches(Some("v"), "mods/a.jar"));
        assert!(!removed.matches(None, "mods/a.jar"));
    }

    #[test]
    fn only_addon_archives_are_removed_paths() {
        assert!(RemovedFile::path("mods", "a.jar").is_some());
        assert!(RemovedFile::path("config", "a.jar").is_none());
        assert!(RemovedFile::path("mods", "a.txt").is_none());
        assert!(RemovedFile::path("mods", "../a.jar").is_none());
        assert!(RemovedFile::path("mods", "a\u{7}.jar").is_none());
    }
}
