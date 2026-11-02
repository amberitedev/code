//! What a push reads from the owner's content: which files go by their Modrinth version, which
//! are carried as files, and which files of the share's modpack members no longer get. Each side
//! reads its own rows (the App its content entries, a Core its server's files) into
//! [`ContentRow`]s and decides which modpack files are off for members; the rules for turning
//! them into a version's content are here, once.

use std::collections::HashSet;

use crate::install::ModpackFile;
use crate::versions::RemovedFile;

/// One content file members get from the owner's instance or server, as its side reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentRow {
    /// Where its bytes are, relative to the instance or server folder.
    pub path: String,
    /// The manifest's name and type: the file name, and `mod`, `datapack` and the like.
    pub file_name: String,
    pub file_type: String,
    /// Its Modrinth version, when members can install it from Modrinth.
    pub version_id: Option<String>,
    /// The share's modpack placed it, so members get it from the modpack while the modpack
    /// still places it.
    pub from_modpack: bool,
}

/// [`read_content`]'s answer: the Modrinth versions members install, and the files the push
/// carries, in the order of the rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadContent {
    pub modrinth_ids: Vec<String>,
    pub carried: Vec<ContentRow>,
}

/// What a push sends of `rows`, for a share whose modpack places `modpack` (empty without one).
/// A file the modpack placed is left to it while the modpack still places it, by its version or
/// at its path (on or off), and so is a Modrinth version the modpack also places. A file
/// Modrinth does not know is carried as it is.
pub fn read_content<S>(
    rows: Vec<ContentRow>,
    modpack: &[ModpackFile<S>],
) -> ReadContent {
    let placed = modpack
        .iter()
        .filter_map(|file| file.version_id.as_deref())
        .collect::<HashSet<_>>();
    let paths = modpack
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    let mut content = ReadContent::default();
    for row in rows {
        let enabled = row.path.strip_suffix(".disabled").unwrap_or(&row.path);
        if row.from_modpack && paths.contains(enabled) {
            continue;
        }
        match &row.version_id {
            Some(version_id) if placed.contains(version_id.as_str()) => {}
            Some(version_id) => content.modrinth_ids.push(version_id.clone()),
            None => content.carried.push(row),
        }
    }
    content
}

/// The files of the share's modpack that members no longer get, as a version lists them: by
/// Modrinth version, or by folder and file name. A file that is neither, such as a config the
/// modpack placed, cannot be named, and is left out. Each one is listed once, in order.
pub fn removed_files<'a, S: 'a>(
    dropped: impl IntoIterator<Item = &'a ModpackFile<S>>,
) -> Vec<RemovedFile> {
    let mut removed = Vec::new();
    for file in dropped {
        let entry = match &file.version_id {
            Some(version_id) => Some(RemovedFile::Version {
                version_id: version_id.clone(),
            }),
            None => file
                .path
                .split_once('/')
                .and_then(|(parent, name)| RemovedFile::path(parent, name)),
        };
        if let Some(entry) = entry
            && !removed.contains(&entry)
        {
            removed.push(entry);
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        path: &str,
        version_id: Option<&str>,
        from_modpack: bool,
    ) -> ContentRow {
        ContentRow {
            path: path.into(),
            file_name: path.rsplit('/').next().unwrap().into(),
            file_type: "mod".into(),
            version_id: version_id.map(str::to_owned),
            from_modpack,
        }
    }

    fn pack_file(path: &str, version_id: Option<&str>) -> ModpackFile {
        ModpackFile {
            path: path.into(),
            sha1: None,
            version_id: version_id.map(str::to_owned),
            alias: None,
            source: (),
        }
    }

    #[test]
    fn the_modpack_s_files_are_left_to_it() {
        let modpack = [
            pack_file("mods/a.jar", Some("a")),
            pack_file("mods/cdn.jar", None),
        ];
        let content = read_content(
            vec![
                row("mods/a.jar", Some("a"), true),
                row("mods/cdn.jar.disabled", None, true),
                // The modpack no longer places it.
                row("mods/leftover.jar", None, true),
                // Installed again by hand: the modpack still places it.
                row("mods/a-copy.jar", Some("a"), false),
                row("mods/b.jar", Some("b"), false),
                row("mods/local.jar", None, false),
            ],
            &modpack,
        );

        assert_eq!(content.modrinth_ids, ["b"]);
        assert_eq!(
            content.carried,
            [
                row("mods/leftover.jar", None, true),
                row("mods/local.jar", None, false),
            ]
        );
    }

    #[test]
    fn removed_files_name_versions_and_addon_archives_once() {
        let dropped = [
            pack_file("mods/a.jar", Some("a")),
            pack_file("mods/other-a.jar", Some("a")),
            pack_file("mods/cdn.jar", None),
            pack_file("config/pack.toml", None),
            pack_file("mods/readme.txt", None),
        ];

        assert_eq!(
            removed_files(&dropped),
            [
                RemovedFile::Version {
                    version_id: "a".into()
                },
                RemovedFile::path("mods", "cdn.jar").unwrap(),
            ]
        );
    }
}
