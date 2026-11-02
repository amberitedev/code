//! Unpacking zip archives into a server folder: pack overrides, uploaded worlds and folders, and
//! the file manager's unarchive. Every entry is checked before anything is written; one entry
//! with `..`, a root, a link, or a name Windows would change refuses the whole archive, and so do
//! listed sizes that do not fit in the disk's free space less what Core keeps free ([`room`]).
//! Listed sizes can lie, so what is unpacked is also counted in the bytes actually inflated: an
//! entry may grow at most `MAX_COMPRESSION_RATIO` times its packed size, and everything unpacked
//! must fit in the room.

use std::io::{self, Read, Seek};

/// What Core may still write on the disk at a path: its free space less the margin it keeps.
pub use bedrock::disk::room;
pub use bedrock::limits::MAX_ARCHIVE_ENTRIES as MAX_ENTRIES;
use bedrock::{
    disk::label,
    limits::{DISK_MARGIN_BYTES, MAX_COMPRESSION_RATIO},
};
use sha1::Digest;

use super::folder::{self, Folder, PathError};

/// An entry this small never counts as a zip bomb, whatever its ratio: a short file of one
/// repeated byte packs to almost nothing, and the disk check still covers it.
const SMALL_ENTRY: u64 = 1024 * 1024;

/// Something does not fit in the disk's free space less what Core keeps free.
#[derive(Debug, thiserror::Error)]
#[error(
    "There is not enough disk space on the Core for {what}{}. The Core keeps {} free.",
    needed.map(|needed| format!(": it needs about {} and {} can be used", label(needed), label(*room))).unwrap_or_default(),
    label(DISK_MARGIN_BYTES)
)]
pub struct NoRoom {
    /// What did not fit, such as "the pack" or "the upload".
    pub what: String,
    /// How much it needs, when that is known before writing.
    pub needed: Option<u64>,
    /// What Core could write.
    pub room: u64,
}

impl NoRoom {
    /// Refuse `what` unless `needed` bytes fit in `room`.
    pub fn check(what: &str, needed: u64, room: u64) -> Result<(), Self> {
        if needed > room {
            return Err(Self {
                what: what.to_owned(),
                needed: Some(needed),
                room,
            });
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("the archive has an entry {name} that is refused: {reason}")]
    Entry { name: String, reason: String },
    #[error(transparent)]
    NoRoom(#[from] NoRoom),
    #[error(
        "This archive is a zip bomb: its entry {0} unpacks to more than {MAX_COMPRESSION_RATIO} times its packed size."
    )]
    Bomb(String),
    #[error("the archive has more than {MAX_ENTRIES} entries")]
    TooMany,
    #[error("the archive is damaged: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl ArchiveError {
    /// The archive error a read of [`Inflating`] failed with, or the I/O error it is.
    fn from_read(error: io::Error) -> Self {
        error.downcast::<Self>().unwrap_or_else(Self::Io)
    }
}

/// A file an extraction wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// Where in the server folder.
    pub path: String,
    pub sha1: String,
}

/// The checked path of every entry, in archive order, with whether it is a folder. Refuses the
/// archive on the first bad entry, and one with more than [`MAX_ENTRIES`].
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

/// Unpack every entry `place` maps to a path into `folder`, and return what was written, at most
/// `room` bytes in all (see [`room`]). `place` gets each checked entry path, once and before
/// anything is written, and returns where it goes, or `None` to skip it. Refuses the whole archive
/// before writing anything if an entry is bad or the entries' listed sizes do not fit in `room`.
/// Listed sizes can lie, so what is inflated is still counted while writing, and an archive that
/// lied can fail partway with its earlier entries written.
pub fn extract<R: Read + Seek>(
    folder: &Folder,
    archive: &mut zip::ZipArchive<R>,
    mut room: u64,
    mut place: impl FnMut(&str) -> Option<String>,
) -> Result<Vec<Placed>, ArchiveError> {
    let mut targets = Vec::new();
    let mut listed = 0u64;
    for (index, (path, is_dir)) in entries(archive)?.into_iter().enumerate() {
        let Some(target) = place(&path) else {
            continue;
        };
        if !is_dir {
            listed = listed.saturating_add(archive.by_index_raw(index)?.size());
        }
        targets.push((index, path, target, is_dir));
    }
    NoRoom::check("the archive", listed, room)?;
    let mut placed = Vec::new();
    for (index, path, target, is_dir) in targets {
        if is_dir {
            folder.create_dir_all(&target)?;
            continue;
        }
        let mut reader =
            Hashing::new(Inflating::new(archive.by_index(index)?, &path, room));
        // Past a limit the read fails, so the write never replaces what was there.
        let written = folder
            .write(&target, &mut reader)
            .map_err(ArchiveError::from_read)?;
        room -= written;
        placed.push(Placed {
            path: target,
            sha1: reader.finish(),
        });
    }
    Ok(placed)
}

/// The SHA-1 and size of the entry at `index`, named `name`, read without unpacking it. The size
/// is what the entry actually inflates to, not what the archive claims; one that grows like a
/// zip bomb is refused. What it inflates to is drawn from `budget`, so hashing every entry of an
/// archive reads no more than the archive could unpack, and one past it is refused.
pub fn entry_sha1<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    index: usize,
    name: &str,
    budget: &mut u64,
) -> Result<(String, u64), ArchiveError> {
    let mut reader =
        Hashing::new(Inflating::new(archive.by_index(index)?, name, *budget));
    let size = io::copy(&mut reader, &mut io::sink())
        .map_err(ArchiveError::from_read)?;
    *budget -= size;
    Ok((reader.finish(), size))
}

/// An entry being inflated. Its read fails once it grows like a zip bomb or past the room there
/// is, so a `Folder::write` of it fails before its file is renamed over anything.
pub struct Inflating<R> {
    inner: R,
    name: String,
    /// What it may still grow before it counts as a zip bomb.
    bomb: u64,
    room: u64,
}

impl<'a, R: Read> Inflating<zip::read::ZipFile<'a, R>> {
    pub fn new(
        entry: zip::read::ZipFile<'a, R>,
        name: &str,
        room: u64,
    ) -> Self {
        let bomb = entry
            .compressed_size()
            .saturating_mul(MAX_COMPRESSION_RATIO)
            .max(SMALL_ENTRY);
        Self {
            inner: entry,
            name: name.to_owned(),
            bomb,
            room,
        }
    }
}

impl<R: Read> Read for Inflating<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        let grown = read as u64;
        if grown > self.bomb {
            return Err(io::Error::other(ArchiveError::Bomb(
                self.name.clone(),
            )));
        }
        if grown > self.room {
            return Err(io::Error::other(ArchiveError::NoRoom(NoRoom {
                what: format!("the archive entry {}", self.name),
                needed: None,
                room: self.room,
            })));
        }
        self.bomb -= grown;
        self.room -= grown;
        Ok(read)
    }
}

/// Reads at most `left` more bytes and fails the read past them, so a `Folder::write` of it fails
/// before its file is renamed over anything already there. For an entry whose size was already
/// counted, which must not have changed since.
pub struct AtMost<R> {
    pub inner: R,
    pub left: u64,
}

impl<R: Read> Read for AtMost<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.left = self.left.checked_sub(read as u64).ok_or_else(|| {
            io::Error::other("the archive entry grew since it was checked")
        })?;
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

    fn anywhere(path: &str) -> Option<String> {
        Some(path.to_owned())
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
                extract(&folder, &mut zip, u64::MAX, anywhere).unwrap_err();
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
            extract(&folder, &mut zip, u64::MAX, anywhere),
            Err(ArchiveError::Entry { .. })
        ));
    }

    #[test]
    fn an_archive_listed_past_the_room_is_refused_with_nothing_written() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(dir.path().join("config/a.toml"), "original").unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        // Each entry fits on its own; together they do not.
        let mut zip = archive(|writer| {
            file(writer, "config/b.toml", &[b'b'; 10]);
            file(writer, "config/a.toml", &[b'a'; 10]);
        });
        let error = extract(&folder, &mut zip, 15, anywhere).unwrap_err();
        assert!(
            matches!(
                error,
                ArchiveError::NoRoom(NoRoom {
                    needed: Some(20),
                    room: 15,
                    ..
                })
            ),
            "{error}"
        );
        assert!(error.to_string().starts_with(
            "There is not enough disk space on the Core for the archive: it needs about"
        ));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config/a.toml")).unwrap(),
            "original"
        );
        assert_eq!(
            std::fs::read_dir(dir.path().join("config"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn no_room_says_what_is_needed_and_what_can_be_used() {
        const GB: u64 = 1024 * 1024 * 1024;
        assert!(NoRoom::check("the pack", GB, GB).is_ok());
        assert_eq!(
            NoRoom::check("the pack", 2 * GB, GB)
                .unwrap_err()
                .to_string(),
            "There is not enough disk space on the Core for the pack: it needs about 2.0 GB and 1.0 GB can be used. The Core keeps 256 MB free."
        );
    }

    #[test]
    fn an_entry_that_grows_like_a_zip_bomb_is_refused() {
        let deflated = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut zip = archive(|writer| {
            writer.start_file("overrides/zeros", deflated).unwrap();
            writer
                .write_all(&vec![0; 4 * SMALL_ENTRY as usize])
                .unwrap();
            writer.start_file("overrides/small", deflated).unwrap();
            writer.write_all(&[0; 1000]).unwrap();
        });
        let mut budget = u64::MAX;
        assert!(matches!(
            entry_sha1(&mut zip, 0, "overrides/zeros", &mut budget),
            Err(ArchiveError::Bomb(_))
        ));
        // A small entry passes whatever its ratio.
        assert_eq!(
            entry_sha1(&mut zip, 1, "overrides/small", &mut budget)
                .unwrap()
                .1,
            1000
        );
    }

    #[test]
    fn hashing_entries_draws_on_one_budget() {
        let mut zip = archive(|writer| {
            file(writer, "overrides/a", &[b'a'; 10]);
            file(writer, "overrides/b", &[b'b'; 10]);
        });
        let mut budget = 15;
        assert_eq!(
            entry_sha1(&mut zip, 0, "overrides/a", &mut budget)
                .unwrap()
                .1,
            10
        );
        assert_eq!(budget, 5);
        assert!(matches!(
            entry_sha1(&mut zip, 1, "overrides/b", &mut budget),
            Err(ArchiveError::NoRoom(_))
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
        let placed = extract(&folder, &mut zip, u64::MAX, |path| {
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
