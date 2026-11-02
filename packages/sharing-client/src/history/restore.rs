//! What a share update may replace on its source or a member: a member's copy of shared content
//! always, and the owner's own content only when restoring, and then only what the share carried
//! at the last push or carries in the restored version. What the owner added since the last push
//! is kept, so a restore never deletes work the owner has not pushed. The App and Core both
//! decide it here; each reads its own rows into it.

use std::collections::HashSet;
use std::hash::Hash;

/// Where content on disk came from, as an update weighs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Installed from the share: a member's copy.
    Share,
    /// Put there by the person whose content it is.
    Own,
    /// Placed by a modpack, which places its own files.
    Pack,
}

/// What the share carried at the last push or carries in the restored version: the owner's own
/// content a restore may replace. `K` is each side's key for a carried file, such as its type and
/// name.
#[derive(Debug, Clone)]
pub struct Carried<K> {
    pub version_ids: HashSet<String>,
    /// The projects of `version_ids`, so a mod updated since the push is replaced too. Only
    /// Modrinth knows them, so each side fills them in.
    pub project_ids: HashSet<String>,
    pub files: HashSet<K>,
}

impl<K: Eq + Hash> Carried<K> {
    /// The versions and files of the last push and the restored version, without projects.
    pub fn new(
        version_ids: impl IntoIterator<Item = String>,
        files: impl IntoIterator<Item = K>,
    ) -> Self {
        Self {
            version_ids: version_ids.into_iter().collect(),
            project_ids: HashSet::new(),
            files: files.into_iter().collect(),
        }
    }

    pub fn carries(
        &self,
        version_id: Option<&str>,
        project_id: Option<&str>,
        file: &K,
    ) -> bool {
        version_id.is_some_and(|id| self.version_ids.contains(id))
            || project_id.is_some_and(|id| self.project_ids.contains(id))
            || self.files.contains(file)
    }
}

/// Whether an update replaces content of `origin` with `version_id`, `project_id` and carried
/// file key `file`. `restore` is what the share carried, when the owner restores a version, and
/// `None` for a member's update, which keeps everything the member added themselves.
pub fn replaced<K: Eq + Hash>(
    origin: Origin,
    restore: Option<&Carried<K>>,
    version_id: Option<&str>,
    project_id: Option<&str>,
    file: &K,
) -> bool {
    match (origin, restore) {
        (Origin::Share, _) => true,
        (Origin::Own, Some(carried)) => {
            carried.carries(version_id, project_id, file)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_restore_keeps_what_the_owner_added_since_the_last_push() {
        let mut carried =
            Carried::new(["pushed".to_owned()], [("mod", "pushed.jar")]);
        carried.project_ids.insert("pushed-project".to_owned());
        let own = |version, project, file| {
            replaced(Origin::Own, Some(&carried), version, project, &file)
        };

        // Shared: in the last push or the restored version.
        assert!(own(Some("pushed"), Some("other"), ("mod", "a.jar")));
        assert!(own(Some("newer"), Some("pushed-project"), ("mod", "a.jar")));
        assert!(own(None, None, ("mod", "pushed.jar")));
        // Added since the last push: kept.
        assert!(!own(Some("new"), Some("new-project"), ("mod", "new.jar")));
        assert!(!own(None, None, ("mod", "unpushed.jar")));
        // A pack's files are the pack's.
        assert!(!replaced(
            Origin::Pack,
            Some(&carried),
            Some("pushed"),
            None,
            &("mod", "a.jar")
        ));
        // A member keeps their own additions; shared content is always replaced.
        assert!(!replaced(
            Origin::Own,
            None::<&Carried<(&str, &str)>>,
            Some("pushed"),
            Some("pushed-project"),
            &("mod", "a.jar")
        ));
        assert!(replaced(
            Origin::Share,
            None::<&Carried<(&str, &str)>>,
            None,
            None,
            &("mod", "a.jar")
        ));
    }
}
