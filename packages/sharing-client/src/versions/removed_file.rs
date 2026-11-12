use contracts::backend::sharing::RemovedFile;

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

/// `parent/filename` as a removed path, or `None` when it is not one: one plain file name of an
/// addon archive in an addon folder.
pub fn removed_path(parent: &str, filename: &str) -> Option<RemovedFile> {
    let removed = RemovedFile::Path {
        parent: parent.to_owned(),
        filename: filename.to_owned(),
    };
    removed.validate().ok().map(|()| removed)
}

/// How the App, Core and backend read a [`RemovedFile`].
pub trait RemovedFileRules {
    /// Whether this names the modpack file at `path`, with its Modrinth `version_id` when
    /// Modrinth knows it. A file turned off (`.disabled`) is the same file as when it is on.
    fn matches(&self, version_id: Option<&str>, path: &str) -> bool;

    /// Checks a removed path as the backend does: an addon folder, and one plain file name of
    /// an addon archive without control characters.
    fn validate(&self) -> Result<(), InvalidRemovedFile>;
}

impl RemovedFileRules for RemovedFile {
    fn matches(&self, version_id: Option<&str>, path: &str) -> bool {
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

    fn validate(&self) -> Result<(), InvalidRemovedFile> {
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
        let removed = removed_path("mods", "a.jar.disabled").unwrap();
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
        assert!(removed_path("mods", "a.jar").is_some());
        assert!(removed_path("config", "a.jar").is_none());
        assert!(removed_path("mods", "a.txt").is_none());
        assert!(removed_path("mods", "../a.jar").is_none());
        assert!(removed_path("mods", "a\u{7}.jar").is_none());
    }
}
