//! Unpacking zip archives into a server folder: pack overrides, uploaded worlds and folders, and
//! the file manager's unarchive. Every entry is checked before anything is written; one entry
//! with `..`, a root, a link, or a name Windows would change refuses the whole archive. What is
//! unpacked is capped by the bytes actually written, not by what the archive claims.

use std::io::{self, Read, Seek};

use sha1::Digest;

use super::folder::{self, Folder, PathError};

/// The most an archive may unpack to.
pub const MAX_UNPACKED: u64 = 8 * 1024 * 1024 * 1024;
/// The most entries an archive may have.
pub const MAX_ENTRIES: usize = 200_000;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("the archive has an entry {name} that is refused: {reason}")]
    Entry { name: String, reason: String },
    #[error("the archive unpacks to more than {} GB", MAX_UNPACKED / 1024 / 1024 / 1024)]
    TooLarge,
    #[error("the archive has more than {MAX_ENTRIES} entries")]
    TooMany,
    #[error("the archive is damaged: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// A file an extraction wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// Where in the server folder.
    pub path: String,
    pub sha1: String,
}

/// The checked path of every entry, in archive order, with whether it is a folder. Refuses the
/// archive on the first bad entry.
pub fn entries<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<(String, bool)>, ArchiveError> {
    if archive.len() > MAX_ENTRIES {
        return Err(ArchiveError::TooMany);
    }
    let mut checked = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index)?;
        let name = entry.name()?.into_owned();
        let refuse = |reason: String| ArchiveError::Entry {
            name: name.clone(),
            reason,
        };
        if entry.is_symlink() {
            return Err(refuse("it is a link".into()));
        }
        let is_dir = entry.is_dir();
        let path =
            folder::clean(name.trim_end_matches('/')).map_err(|error| {
                refuse(match error {
                    PathError::Escapes(_) => "it leaves the folder".into(),
                    PathError::Invalid(_) => "it is not a usable name".into(),
                })
            })?;
        checked.push((path, is_dir));
    }
    Ok(checked)
}

/// Unpack every entry `place` maps to a path into `folder`, and return what was written.
/// `place` gets each checked entry path and returns where it goes, or `None` to skip it.
/// Refuses the whole archive before writing anything if an entry is bad.
pub fn extract<R: Read + Seek>(
    folder: &Folder,
    archive: &mut zip::ZipArchive<R>,
    place: impl FnMut(&str) -> Option<String>,
) -> Result<Vec<Placed>, ArchiveError> {
    extract_within(folder, archive, MAX_UNPACKED, place)
}

/// [`extract`], unpacking at most `budget` bytes in all.
fn extract_within<R: Read + Seek>(
    folder: &Folder,
    archive: &mut zip::ZipArchive<R>,
    mut budget: u64,
    mut place: impl FnMut(&str) -> Option<String>,
) -> Result<Vec<Placed>, ArchiveError> {
    let entries = entries(archive)?;
    let mut placed = Vec::new();
    for (index, (path, is_dir)) in entries.into_iter().enumerate() {
        let Some(target) = place(&path) else {
            continue;
        };
        if is_dir {
            folder.create_dir_all(&target)?;
            continue;
        }
        let entry = archive.by_index(index)?;
        let mut reader = Hashing::new(AtMost {
            inner: entry,
            left: budget,
        });
        // Past the budget the read fails, so the write never replaces what was there.
        let written = match folder.write(&target, &mut reader) {
            Err(error)
                if error.get_ref().is_some_and(|inner| {
                    matches!(
                        inner.downcast_ref::<ArchiveError>(),
                        Some(ArchiveError::TooLarge)
                    )
                }) =>
            {
                return Err(ArchiveError::TooLarge);
            }
            written => written?,
        };
        budget -= written;
        placed.push(Placed {
            path: target,
            sha1: reader.finish(),
        });
    }
    Ok(placed)
}

/// The SHA-1 and size of one entry, read without unpacking it. The size is what the entry
/// actually inflates to, not what the archive claims, and is taken from `budget`, shared by
/// every entry read: inflating past what is left stops at once.
pub fn entry_sha1<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    index: usize,
    budget: &mut u64,
) -> Result<(String, u64), ArchiveError> {
    let mut reader = Hashing::new(archive.by_index(index)?.take(*budget + 1));
    let size = io::copy(&mut reader, &mut io::sink())?;
    *budget = budget.checked_sub(size).ok_or(ArchiveError::TooLarge)?;
    Ok((reader.finish(), size))
}

/// Reads at most `left` more bytes and fails the read past them, so a `Folder::write` of it fails
/// before its file is renamed over anything already there.
pub struct AtMost<R> {
    pub inner: R,
    pub left: u64,
}

impl<R: Read> Read for AtMost<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.left = self
            .left
            .checked_sub(read as u64)
            .ok_or_else(|| io::Error::other(ArchiveError::TooLarge))?;
        Ok(read)
    }
}

/// Hashes what passes through it.
pub struct Hashing<R> {
    inner: R,
    hasher: sha1::Sha1,
}

impl<R: Read> Hashing<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: sha1::Sha1::new(),
        }
    }

    pub fn finish(self) -> String {
        hex::encode(self.hasher.finalize())
    }
}

impl<R: Read> Read for Hashing<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hasher.update(&buf[..read]);
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    fn archive(
        build: impl FnOnce(&mut zip::ZipWriter<Cursor<Vec<u8>>>),
    ) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        build(&mut writer);
        zip::ZipArchive::new(writer.finish().unwrap()).unwrap()
    }

    fn file(
        writer: &mut zip::ZipWriter<Cursor<Vec<u8>>>,
        name: &str,
        body: &[u8],
    ) {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body).unwrap();
    }

    #[test]
    fn entries_that_leave_the_folder_refuse_the_archive() {
        for bad in [
            "../db/copal.db",
            "/etc/cron.d/x",
            "mods/../../x",
            "C:/Windows/x",
            "mods\\..\\x",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let folder = Folder::open(dir.path()).unwrap();
            let mut zip = archive(|writer| {
                file(writer, "config/fine.toml", b"x");
                file(writer, bad, b"payload");
            });
            let error =
                extract(&folder, &mut zip, |path| Some(path.to_owned()))
                    .unwrap_err();
            assert!(
                matches!(error, ArchiveError::Entry { .. }),
                "{bad}: {error}"
            );
            // Nothing was written, not even the entry before it.
            assert_eq!(
                std::fs::read_dir(dir.path()).unwrap().count(),
                0,
                "{bad}"
            );
        }
    }

    #[test]
    fn links_refuse_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        let mut zip = archive(|writer| {
            writer
                .add_symlink(
                    "mods",
                    "/var/lib/copal/db",
                    SimpleFileOptions::default(),
                )
                .unwrap();
        });
        assert!(matches!(
            extract(&folder, &mut zip, |path| Some(path.to_owned())),
            Err(ArchiveError::Entry { .. })
        ));
    }

    #[test]
    fn an_entry_past_the_budget_leaves_the_file_it_would_replace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(dir.path().join("config/a.toml"), "original").unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        let mut zip = archive(|writer| {
            file(writer, "config/a.toml", &[b'x'; 20]);
        });
        let error =
            extract_within(&folder, &mut zip, 10, |path| Some(path.to_owned()))
                .unwrap_err();
        assert!(matches!(error, ArchiveError::TooLarge), "{error}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config/a.toml")).unwrap(),
            "original"
        );
        // Nor is its half-written copy left beside it.
        assert_eq!(
            std::fs::read_dir(dir.path().join("config"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn hashing_entries_draws_on_one_budget() {
        let mut zip = archive(|writer| {
            file(writer, "overrides/a", &[b'a'; 10]);
            file(writer, "overrides/b", &[b'b'; 10]);
        });
        let mut budget = 15;
        assert_eq!(entry_sha1(&mut zip, 0, &mut budget).unwrap().1, 10);
        assert_eq!(budget, 5);
        assert!(matches!(
            entry_sha1(&mut zip, 1, &mut budget),
            Err(ArchiveError::TooLarge)
        ));
    }

    #[test]
    fn entries_land_where_place_says_with_their_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        let mut zip = archive(|writer| {
            writer
                .add_directory(
                    "overrides/config/",
                    SimpleFileOptions::default(),
                )
                .unwrap();
            file(writer, "overrides/config/a.toml", b"x = 1");
            file(writer, "client-overrides/options.txt", b"fov");
        });
        let placed = extract(&folder, &mut zip, |path| {
            path.strip_prefix("overrides/").map(str::to_owned)
        })
        .unwrap();
        assert_eq!(
            placed,
            [Placed {
                path: "config/a.toml".into(),
                sha1: "34bce5f775de97f557a34088509c8bfe1ea17e52".into(),
            }]
        );
        assert!(!dir.path().join("options.txt").exists());
    }
}
