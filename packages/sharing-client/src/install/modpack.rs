use bedrock::state::ProjectType;

use crate::rules::CONFIG_FILE_TYPE;
use crate::versions::{RemovedFile, ShareVersion, VersionContent};

/// A file a version's modpack places, as the caller read it from the modpack: an index entry or
/// an override alike. `S` is how the caller gets its bytes, such as the index entry or the
/// archive entry; the App needs none.
#[derive(Debug, Clone)]
pub struct ModpackFile<S = ()> {
    /// Its path in the instance or server folder.
    pub path: String,
    pub sha1: Option<String>,
    /// Its Modrinth version, when Modrinth knows the file.
    pub version_id: Option<String>,
    /// A second path a removed file may name it by: where hosting puts a file Modrinth lists
    /// only by its file name.
    pub alias: Option<String>,
    pub source: S,
}

impl<S> ModpackFile<S> {
    /// Whether the owner took this file out of the share.
    pub fn is_removed(&self, removed: &[RemovedFile]) -> bool {
        removed.iter().any(|removed| {
            removed.matches(self.version_id.as_deref(), &self.path)
                || self
                    .alias
                    .as_deref()
                    .is_some_and(|alias| removed.matches(None, alias))
        })
    }
}

/// A version's content, split into what its modpack places and what the version adds.
#[derive(Debug, Clone)]
pub struct VersionFiles<S = ()> {
    /// The Modrinth versions the version adds over its modpack, without the modpack itself.
    pub modrinth_ids: Vec<String>,
    /// The modpack's files members get: those the owner did not remove and the version does not
    /// replace with a file of its own.
    pub modpack_files: Vec<ModpackFile<S>>,
}

impl<S> VersionFiles<S> {
    /// Every Modrinth version members end up with, the modpack's included, sorted.
    pub fn all_modrinth_ids(&self) -> Vec<String> {
        let mut ids = self
            .modpack_files
            .iter()
            .filter_map(|file| file.version_id.clone())
            .chain(self.modrinth_ids.iter().cloned())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// The modpack's files Modrinth does not know, which come from the modpack itself.
    pub fn unknown_modpack_files(
        &self,
    ) -> impl Iterator<Item = &ModpackFile<S>> {
        self.modpack_files
            .iter()
            .filter(|file| file.version_id.is_none())
    }
}

/// Where a manifest entry goes in an instance or server folder: a config file at its path, a
/// content file in its type's folder. `None` for a type no folder takes.
pub fn content_path(file_type: &str, file_name: &str) -> Option<String> {
    if file_type == CONFIG_FILE_TYPE {
        return Some(file_name.to_owned());
    }
    ProjectType::from_name(file_type)
        .map(|kind| format!("{}/{file_name}", kind.get_folder()))
}

/// `version`'s content less what its modpack places, from `modpack`, every file of the
/// version's modpack as the caller reads it (empty without a modpack). The one place both sides
/// split a version, for installs, previews and diffs alike.
pub fn version_files<S>(
    version: &ShareVersion,
    modpack: Vec<ModpackFile<S>>,
) -> VersionFiles<S> {
    split_content(
        &version.content,
        version
            .external_files
            .iter()
            .map(|file| (file.file_type.as_str(), file.file_name.as_str())),
        modpack,
    )
}

/// [`version_files`] for content not yet pushed: `carried` names each file it carries by type
/// and name.
pub fn split_content<'a, S>(
    content: &VersionContent,
    carried: impl IntoIterator<Item = (&'a str, &'a str)>,
    modpack: Vec<ModpackFile<S>>,
) -> VersionFiles<S> {
    let modpack_id = content.modpack_id.as_deref().filter(|id| !id.is_empty());
    let carried = carried
        .into_iter()
        .filter_map(|(file_type, file_name)| content_path(file_type, file_name))
        .collect::<std::collections::HashSet<_>>();
    let modpack_files = modpack
        .into_iter()
        .filter(|file| {
            !file.is_removed(&content.removed_files)
                && !carried.contains(&file.path)
        })
        .collect::<Vec<_>>();
    let mut modrinth_ids = content
        .modrinth_ids
        .iter()
        .filter(|id| {
            modpack_id != Some(id.as_str())
                && !modpack_files
                    .iter()
                    .any(|file| file.version_id.as_deref() == Some(id.as_str()))
        })
        .cloned()
        .collect::<Vec<_>>();
    modrinth_ids.sort_unstable();
    modrinth_ids.dedup();
    VersionFiles {
        modrinth_ids,
        modpack_files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::versions::ManifestFile;

    fn version(
        modrinth_ids: &[&str],
        removed: Vec<RemovedFile>,
    ) -> ShareVersion {
        ShareVersion {
            version: 1,
            ready: true,
            content: VersionContent {
                modrinth_ids: modrinth_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect(),
                modpack_id: Some("pack".into()),
                removed_files: removed,
                game_version: "1.21.1".into(),
                loader: "fabric".into(),
                loader_version: "0.16.0".into(),
            },
            root: [0; 32],
            external_files: vec![ManifestFile {
                file_name: "carried.jar".into(),
                file_type: "mod".into(),
                file_size: 1,
                sha1: String::new(),
                sha256: [0; 32],
                object: [0; 32],
                object_size: 17,
                key: None,
            }],
            pool: None,
            store: None,
            backtrack_version: None,
        }
    }

    fn file(path: &str, version_id: Option<&str>) -> ModpackFile {
        ModpackFile {
            path: path.into(),
            sha1: None,
            version_id: version_id.map(str::to_owned),
            alias: None,
            source: (),
        }
    }

    #[test]
    fn the_modpack_keeps_its_files_modrinth_does_not_know() {
        let version = version(
            &["pack", "a", "extra"],
            vec![RemovedFile::path("mods", "gone.jar").unwrap()],
        );
        let files = version_files(
            &version,
            vec![
                file("mods/a.jar", Some("a")),
                file("mods/cdn.jar", None),
                file("config/pack.toml", None),
                file("mods/gone.jar", None),
                file("mods/carried.jar", Some("old")),
            ],
        );

        assert_eq!(files.modrinth_ids, ["extra"]);
        assert_eq!(files.all_modrinth_ids(), ["a", "extra"]);
        let unknown = files
            .unknown_modpack_files()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(unknown, ["mods/cdn.jar", "config/pack.toml"]);
    }
}
