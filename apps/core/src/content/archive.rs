//! Unpacking zip archives into a server folder: pack overrides, uploaded worlds and folders, and
//! the file manager's unarchive. Every entry is checked before anything is written; one entry
//! with `..`, a root, a link, or a name Windows would change refuses the whole archive. So is an
//! archive that would not fit on the disk, judged by the sizes it lists: the zip reader fails an
//! entry that inflates past its listed size, so those are a true bound.

use std::io::{self, Read, Seek};

use sha1::Digest;

use super::folder::{self, Folder, PathError};
use crate::backups::space::{self, NoSpace};

/// The most entries an archive may have.
pub const MAX_ENTRIES: usize = 200_000;
/// About the most deflate can shrink data. An archive listing more than this times its own size
/// has entries that share their data, which only a zip bomb does.
const MAX_RATIO: u64 = 1032;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("the archive has an entry {name} that is refused: {reason}")]
    Entry { name: String, reason: String },
    #[error("the archive has more than {MAX_ENTRIES} entries")]
    TooMany,
    #[error(
        "This archive is a zip bomb: {} that would unpack to {}. Nothing was unpacked.",
        space::label(*.packed),
        space::label(*.unpacked)
    )]
    Bomb { packed: u64, unpacked: u64 },
    #[error(transparent)]
    Space(#[from] NoSpace),
    #[error("the archive is damaged: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Refuse unpacking `unpacked` bytes, as an archive of `packed` bytes lists them, onto a disk
/// with `free` bytes. One that does not fit is called a zip bomb when no real archive could
/// shrink that much; either way nothing is read or written yet.
pub fn check_room(
    unpacked: u64,
    packed: u64,
    free: u64,
) -> Result<(), ArchiveError> {
    space::enough_space(unpacked, free).map_err(|no_space| {
        if unpacked / packed.max(1) > MAX_RATIO {
            ArchiveError::Bomb { packed, unpacked }
        } else {
            no_space.into()
        }
    })
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
/// `packed` is the archive's size. Refuses the whole archive before writing anything if an
/// entry is bad or what it places would not fit on the disk.
pub fn extract<R: Read + Seek>(
    folder: &Folder,
    archive: &mut zip::ZipArchive<R>,
    packed: u64,
    mut place: impl FnMut(&str) -> Option<String>,
) -> Result<Vec<Placed>, ArchiveError> {
    let mut targets = Vec::new();
    let mut unpacked = 0u64;
    for (index, (path, is_dir)) in entries(archive)?.into_iter().enumerate() {
        let Some(target) = place(&path) else {
            continue;
        };
        unpacked = unpacked.saturating_add(archive.by_index_raw(index)?.size());
        targets.push((index, target, is_dir));
    }
    check_room(unpacked, packed, space::free(folder.root())?)?;
    let mut placed = Vec::new();
    for (index, target, is_dir) in targets {
        if is_dir {
            folder.create_dir_all(&target)?;
            continue;
        }
        // An entry that inflates past its listed size fails the read, so the write never
        // replaces what was there.
        let mut reader = Hashing::new(archive.by_index(index)?);
        folder.write(&target, &mut reader)?;
        placed.push(Placed {
            path: target,
            sha1: reader.finish(),
        });
    }
    Ok(placed)
}

/// The SHA-1 and size of one entry, read without unpacking it.
pub fn entry_sha1<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    index: usize,
) -> Result<(String, u64), ArchiveError> {
    let mut reader = Hashing::new(archive.by_index(index)?);
    let size = io::copy(&mut reader, &mut io::sink())?;
    Ok((reader.finish(), size))
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
                extract(&folder, &mut zip, 0, |path| Some(path.to_owned()))
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
            extract(&folder, &mut zip, 0, |path| Some(path.to_owned())),
            Err(ArchiveError::Entry { .. })
        ));
    }

    #[test]
    fn an_entry_that_inflates_past_its_listed_size_leaves_the_file_it_would_replace()
     {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(dir.path().join("config/a.toml"), "original").unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        // Stored, so the listed sizes sit as plain numbers: 20 bytes, claimed as 10.
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "config/a.toml",
                SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(&[b'x'; 20]).unwrap();
        let mut bytes = writer.finish().unwrap().into_inner();
        for (signature, offset) in [(b"PK\x03\x04", 22), (b"PK\x01\x02", 24)] {
            let at = bytes
                .windows(4)
                .position(|window| window == signature)
                .unwrap();
            bytes[at + offset..at + offset + 4]
                .copy_from_slice(&10u32.to_le_bytes());
        }
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        extract(&folder, &mut zip, 0, |path| Some(path.to_owned()))
            .unwrap_err();
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
    fn an_archive_that_does_not_fit_is_a_bomb_only_past_what_deflate_can_do() {
        const GB: u64 = 1024 * 1024 * 1024;
        assert!(check_room(GB, GB, 10 * GB).is_ok());
        assert!(matches!(
            check_room(20 * GB, 4 * GB, 10 * GB),
            Err(ArchiveError::Space(_))
        ));
        assert!(matches!(
            check_room(20 * GB, 1024 * 1024, 10 * GB),
            Err(ArchiveError::Bomb { .. })
        ));
        // A bomb that fits harms nothing, so it is unpacked like any archive.
        assert!(check_room(GB, 1024, 10 * GB).is_ok());
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
        let placed = extract(&folder, &mut zip, 0, |path| {
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
