//! The files an owner can pick for a push: every file a [`ShareScope`] reaches in the instance or
//! server folder, each marked blocked or shared. Each side reads its own folder through
//! [`ShareFolder`]; the walk and the marks are here, once.

use std::collections::BTreeSet;

use super::ShareScope;

/// An entry of a folder, as a walk sees it without following links.
pub enum FolderEntry {
    Dir(String),
    File { name: String, size: u64 },
}

/// A folder as [`list_shareable`] reads it, by paths relative to it with `/` separators.
pub trait ShareFolder {
    /// The directories and plain files in the directory at `path` (`""` for the folder itself).
    /// Links and anything else are left out.
    fn entries(&self, path: &str) -> std::io::Result<Vec<FolderEntry>>;

    /// Up to [`super::SCAN_HEAD_SIZE`] leading bytes of the file at `path`, which tell an
    /// executable apart. Empty when it cannot be read.
    fn head(&self, path: &str) -> Vec<u8>;
}

/// A file the owner can pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareableFile {
    pub path: String,
    pub size: u64,
    /// The scope reaches it, but a share cannot carry it.
    pub blocked: bool,
    /// The share's latest version carries it.
    pub shared: bool,
}

/// [`list_shareable`]'s answer, sorted by path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShareableFiles {
    pub files: Vec<ShareableFile>,
    /// The walk stopped at its limit, so some files are not listed.
    pub truncated: bool,
}

/// Every file `scope` reaches in `folder`, at most `limit` of them, marked shared when its path
/// is in `shared` (the configs of the share's latest version). The folder itself must be
/// readable; a directory inside it that cannot be opened, such as a link, is skipped.
pub fn list_shareable(
    scope: ShareScope,
    folder: &impl ShareFolder,
    shared: &BTreeSet<String>,
    limit: usize,
) -> std::io::Result<ShareableFiles> {
    let mut listed = ShareableFiles::default();
    let mut dirs = vec![String::new()];
    while let Some(dir) = dirs.pop() {
        let entries = match folder.entries(&dir) {
            Ok(entries) => entries,
            Err(_) if !dir.is_empty() => continue,
            Err(error) => return Err(error),
        };
        let child = |name: &str| {
            if dir.is_empty() {
                name.to_owned()
            } else {
                format!("{dir}/{name}")
            }
        };
        for entry in entries {
            match entry {
                FolderEntry::Dir(name) => {
                    let path = child(&name);
                    if scope.enters(&path) {
                        dirs.push(path);
                    }
                }
                FolderEntry::File { name, size } => {
                    let path = child(&name);
                    if !scope.reaches(&path) {
                        continue;
                    }
                    if listed.files.len() == limit {
                        listed.truncated = true;
                        break;
                    }
                    listed.files.push(ShareableFile {
                        blocked: scope.blocked(&path, &folder.head(&path)),
                        shared: shared.contains(&path),
                        size,
                        path,
                    });
                }
            }
        }
        if listed.truncated {
            break;
        }
    }
    listed
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(listed)
}

/// A folder on disk, read with `std::fs`, for a side whose folder needs no more care than that.
pub struct DiskFolder<'a>(pub &'a std::path::Path);

impl ShareFolder for DiskFolder<'_> {
    fn entries(&self, path: &str) -> std::io::Result<Vec<FolderEntry>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(self.0.join(path))? {
            let entry = entry?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let kind = entry.file_type()?;
            if kind.is_dir() {
                entries.push(FolderEntry::Dir(name));
            } else if kind.is_file() {
                let size = entry.metadata()?.len();
                entries.push(FolderEntry::File { name, size });
            }
        }
        Ok(entries)
    }

    fn head(&self, path: &str) -> Vec<u8> {
        use std::io::Read;

        let mut head = Vec::with_capacity(super::SCAN_HEAD_SIZE);
        if let Ok(file) = std::fs::File::open(self.0.join(path)) {
            let _ = file
                .take(super::SCAN_HEAD_SIZE as u64)
                .read_to_end(&mut head);
        }
        head
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_lists_what_the_scope_reaches_and_marks_it() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir_all(root.join("config/.hidden")).unwrap();
        std::fs::create_dir_all(root.join("mods")).unwrap();
        std::fs::write(root.join("config/a.toml"), "x = 1").unwrap();
        std::fs::write(root.join("config/b.jar"), "jar").unwrap();
        std::fs::write(
            root.join("config/c.json"),
            b"MZ\x90\x00\x03\x00\x00\x00",
        )
        .unwrap();
        std::fs::write(root.join("config/.hidden/d.toml"), "x = 1").unwrap();
        std::fs::write(root.join("mods/e.jar"), "jar").unwrap();
        std::fs::write(root.join("options.txt"), "fov:90").unwrap();
        let shared = BTreeSet::from(["config/a.toml".to_owned()]);
        let marks = |listed: ShareableFiles| {
            listed
                .files
                .into_iter()
                .map(|file| (file.path, file.blocked, file.shared))
                .collect::<Vec<_>>()
        };

        let server =
            list_shareable(ShareScope::Server, &DiskFolder(root), &shared, 10)
                .unwrap();
        assert_eq!(
            marks(server),
            [
                ("config/a.toml".to_owned(), false, true),
                ("config/b.jar".to_owned(), true, false),
                ("config/c.json".to_owned(), true, false),
            ]
        );

        let instance = ShareScope::Instance {
            exportable: |_| true,
        };
        let listed =
            list_shareable(instance, &DiskFolder(root), &shared, 10).unwrap();
        assert!(!listed.truncated);
        assert_eq!(
            listed
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            [
                "config/.hidden/d.toml",
                "config/a.toml",
                "config/b.jar",
                "config/c.json",
                "options.txt",
            ]
        );

        let truncated =
            list_shareable(instance, &DiskFolder(root), &shared, 2).unwrap();
        assert!(truncated.truncated);
        assert_eq!(truncated.files.len(), 2);
    }
}
