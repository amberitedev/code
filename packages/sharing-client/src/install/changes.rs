//! How content on disk becomes a version's content without a full reinstall: which Modrinth
//! versions go, which are updated in place, which come, and which carried files are removed,
//! rewritten or added. The App applies member updates and owner restores with it, and Core
//! applies restores of a server's own share. Which content counts as current (what an update may
//! replace, see [`crate::history::restore`]) and how each change is written stay with each side.

use std::collections::HashMap;
use std::hash::Hash;

/// A Modrinth version as content: on disk now, with the side's own `item` (where it is), or
/// wanted by the version (`T = ()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentProject<T = ()> {
    pub project_id: String,
    pub version_id: String,
    pub item: T,
}

/// A carried file on disk now: its SHA-1, `None` while the file is missing, and the side's own
/// `item`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentFile<T> {
    pub sha1: Option<String>,
    pub item: T,
}

/// A carried file the version wants: its SHA-1 as the share sent it, when known, and the side's
/// own `file`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredFile<D> {
    pub sha1: Option<String>,
    pub file: D,
}

/// What [`content_changes`] does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentChanges<P, F, D> {
    pub project_removals: Vec<ContentProject<P>>,
    /// A current version replaced by the one version of the same project the version wants.
    pub project_updates: Vec<(ContentProject<P>, ContentProject)>,
    pub project_additions: Vec<ContentProject>,
    pub file_removals: Vec<F>,
    /// Wanted files on disk with other bytes, or missing.
    pub file_updates: Vec<D>,
    pub file_additions: Vec<D>,
}

impl<P, F, D> Default for ContentChanges<P, F, D> {
    fn default() -> Self {
        Self {
            project_removals: Vec::new(),
            project_updates: Vec::new(),
            project_additions: Vec::new(),
            file_removals: Vec::new(),
            file_updates: Vec::new(),
            file_additions: Vec::new(),
        }
    }
}

/// How `current` content (projects by version id, files by the side's key) becomes `desired`.
/// A wanted version of a project that has exactly one current version going, and no other
/// wanted version, updates that one in place; otherwise it is added beside the removals. A file
/// is unchanged only while its bytes match what the version sent.
pub fn content_changes<K: Eq + Hash, P, F, D>(
    current_projects: HashMap<String, ContentProject<P>>,
    current_files: HashMap<K, CurrentFile<F>>,
    desired_projects: HashMap<String, ContentProject>,
    desired_files: HashMap<K, DesiredFile<D>>,
) -> ContentChanges<P, F, D> {
    let mut changes = ContentChanges::default();
    let mut additions = Vec::new();
    let mut kept = HashMap::new();
    for (version_id, project) in current_projects {
        if desired_projects.contains_key(&version_id) {
            kept.insert(version_id, project);
        } else {
            changes.project_removals.push(project);
        }
    }
    for (version_id, desired) in desired_projects {
        if !kept.contains_key(&version_id) {
            additions.push(desired);
        }
    }
    while let Some(desired) = additions.pop() {
        let replacements = changes
            .project_removals
            .iter()
            .enumerate()
            .filter(|(_, current)| current.project_id == desired.project_id)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let unambiguous = !additions
            .iter()
            .any(|other| other.project_id == desired.project_id)
            && !changes
                .project_additions
                .iter()
                .any(|other| other.project_id == desired.project_id);
        if replacements.len() == 1 && unambiguous {
            let current = changes.project_removals.remove(replacements[0]);
            changes.project_updates.push((current, desired));
        } else {
            changes.project_additions.push(desired);
        }
    }

    let mut current_files = current_files;
    for (key, desired) in desired_files {
        match current_files.remove(&key) {
            Some(current)
                if current
                    .sha1
                    .as_deref()
                    .zip(desired.sha1.as_deref())
                    .is_some_and(|(current, desired)| {
                        current.eq_ignore_ascii_case(desired)
                    }) => {}
            Some(_) => changes.file_updates.push(desired.file),
            None => changes.file_additions.push(desired.file),
        }
    }
    changes.file_removals =
        current_files.into_values().map(|file| file.item).collect();
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project<T>(
        project_id: &str,
        version_id: &str,
        item: T,
    ) -> (String, ContentProject<T>) {
        (
            version_id.into(),
            ContentProject {
                project_id: project_id.into(),
                version_id: version_id.into(),
                item,
            },
        )
    }

    fn current(
        sha1: Option<&str>,
        item: &'static str,
    ) -> CurrentFile<&'static str> {
        CurrentFile {
            sha1: sha1.map(Into::into),
            item,
        }
    }

    fn desired(
        sha1: Option<&str>,
        file: &'static str,
    ) -> DesiredFile<&'static str> {
        DesiredFile {
            sha1: sha1.map(Into::into),
            file,
        }
    }

    #[test]
    fn versions_are_kept_updated_added_and_removed() {
        let changes = content_changes::<&str, _, &str, &str>(
            HashMap::from([
                project("same", "same-1", "mods/same.jar"),
                project("sodium", "sodium-2", "mods/sodium.jar"),
                project("gone", "gone-1", "mods/gone.jar"),
                // Two wanted versions of one project: not an update.
                project("twin", "twin-1", "mods/twin.jar"),
            ]),
            HashMap::new(),
            HashMap::from([
                project("same", "same-1", ()),
                project("sodium", "sodium-1", ()),
                project("new", "new-1", ()),
                project("twin", "twin-2", ()),
                project("twin", "twin-3", ()),
            ]),
            HashMap::new(),
        );
        let mut removed = changes
            .project_removals
            .iter()
            .map(|project| project.item)
            .collect::<Vec<_>>();
        removed.sort_unstable();
        assert_eq!(removed, ["mods/gone.jar", "mods/twin.jar"]);
        let updates = changes
            .project_updates
            .iter()
            .map(|(current, desired)| {
                (current.item, desired.version_id.as_str())
            })
            .collect::<Vec<_>>();
        assert_eq!(updates, [("mods/sodium.jar", "sodium-1")]);
        let mut added = changes
            .project_additions
            .iter()
            .map(|project| project.version_id.as_str())
            .collect::<Vec<_>>();
        added.sort_unstable();
        assert_eq!(added, ["new-1", "twin-2", "twin-3"]);
    }

    #[test]
    fn a_file_is_unchanged_only_while_its_bytes_match() {
        let changes = content_changes::<_, (), _, _>(
            HashMap::new(),
            HashMap::from([
                ("same", current(Some("AB"), "same")),
                ("edited", current(Some("x"), "edited")),
                ("missing", current(None, "missing")),
                ("unknown", current(Some("y"), "unknown")),
                ("gone", current(Some("z"), "gone")),
            ]),
            HashMap::new(),
            HashMap::from([
                ("same", desired(Some("ab"), "same")),
                ("edited", desired(Some("y"), "edited")),
                ("missing", desired(Some("m"), "missing")),
                // Without a hash from the share it is treated as changed.
                ("unknown", desired(None, "unknown")),
                ("new", desired(Some("n"), "new")),
            ]),
        );
        let mut updated = changes.file_updates.clone();
        updated.sort_unstable();
        assert_eq!(updated, ["edited", "missing", "unknown"]);
        assert_eq!(changes.file_additions, ["new"]);
        assert_eq!(changes.file_removals, ["gone"]);
    }
}
