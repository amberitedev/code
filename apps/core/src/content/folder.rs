//! A server's folder, as Core opens it. Mods write in it too and can plant links anywhere inside,
//! so Core walks every path one part at a time from a handle on the folder and refuses a link or
//! reparse point at any part, the folder itself included: a link can neither lead out of the
//! folder nor from one place in it to another (`mods` to the world, past the deny-list). Files
//! are opened without following a link and without blocking, and read only once the handle is a
//! plain file, so a FIFO a mod planted cannot hang Core. Paths from the API, packs, and archives
//! are also checked by [`clean`] first, so they fail with a clear reason and mean the same thing
//! on every OS.
//!
//! Files and folders Core creates here stay writable by `copal-servers` (see `daemon::folders`).

use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use cap_fs_ext::DirExt;
use cap_std::{ambient_authority, fs::Dir};

use crate::{installs::download::sha1_of, servers::launch};

/// Why a path is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("{0} leaves the server folder")]
    Escapes(String),
    #[error("{0} is not a usable file name")]
    Invalid(String),
}

/// Check a relative path and return it with forward slashes. Refused: `..`, a root, a drive or
/// stream (`:`), backslashes, empty and `.` parts, names Windows changes or reserves (a trailing
/// dot or space, `CON`, `NUL`, ...), and control characters. The same path means the same file on
/// every OS, so a deny-list match cannot be dodged by spelling.
pub fn clean(path: &str) -> Result<String, PathError> {
    let escapes = || PathError::Escapes(path.to_owned());
    let invalid = || PathError::Invalid(path.to_owned());
    if path.starts_with('/') || path.contains(':') {
        return Err(escapes());
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => return Err(invalid()),
            ".." => return Err(escapes()),
            _ => {}
        }
        let reserved = {
            let stem = part.split('.').next().unwrap_or_default();
            ["CON", "PRN", "AUX", "NUL"]
                .iter()
                .any(|name| stem.eq_ignore_ascii_case(name))
                || matches!(
                    stem.as_bytes(),
                    [a, b, c, digit]
                        if digit.is_ascii_digit()
                            && ([*a, *b, *c].eq_ignore_ascii_case(b"COM")
                                || [*a, *b, *c].eq_ignore_ascii_case(b"LPT"))
                )
        };
        if reserved
            || part.ends_with(['.', ' '])
            || part.chars().any(|c| {
                c.is_control()
                    || matches!(c, '\\' | '<' | '>' | '"' | '|' | '?' | '*')
            })
        {
            return Err(invalid());
        }
        parts.push(part);
    }
    Ok(parts.join("/"))
}

/// A path the App sends: relative to the server folder, a leading `/` allowed. `None` for the
/// folder itself.
pub fn from_api(path: &str) -> Result<Option<String>, PathError> {
    let trimmed = path.trim_start_matches('/').trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(None);
    }
    clean(trimmed).map(Some)
}

/// Files that belong to the server, not to its content: settings, the operator and ban lists,
/// the EULA, and the worlds. Pack overrides and uploads never replace them; the App edits them
/// on purpose through the file and properties routes. Matched without case, because Windows does.
/// A path as a case-insensitive file system compares it: each character by its simple
/// uppercase mapping, the way NTFS and APFS fold names (`ß` stays `ß`; `İ` and `i̇` differ).
pub fn fold_case(path: &str) -> String {
    path.chars()
        .map(|c| {
            let mut upper = c.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(single), None) => single,
                _ => c,
            }
        })
        .collect()
}

/// Whether a server-owned file is at `path` for a server whose world is `level_name`. Compared
/// without case on every OS: protecting `World/` on Linux too costs nothing.
pub fn is_server_owned(path: &str, level_name: &str) -> bool {
    const FILES: [&str; 11] = [
        "server.properties",
        "eula.txt",
        "ops.json",
        "whitelist.json",
        "banned-players.json",
        "banned-ips.json",
        "ops.txt",
        "white-list.txt",
        "banned-players.txt",
        "banned-ips.txt",
        "usercache.json",
    ];
    let path = fold_case(path);
    if FILES.iter().any(|file| fold_case(file) == path) {
        return true;
    }
    let level = level_name.trim_matches('/');
    [
        level.to_owned(),
        format!("{level}_nether"),
        format!("{level}_the_end"),
    ]
    .iter()
    .map(|world| fold_case(world))
    .any(|world| path == world || path.starts_with(&format!("{world}/")))
}

/// The world folder `server.properties` names, `world` by default.
pub fn level_name(server_dir: &Path) -> String {
    launch::read_server_file(&server_dir.join("server.properties"))
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                (launch::property_key(line) == Some("level-name"))
                    .then(|| {
                        line.split_once(['=', ':'])
                            .map(|(_, v)| v.trim().to_owned())
                    })
                    .flatten()
            })
        })
        .and_then(|name| clean(&name).ok())
        .unwrap_or_else(|| "world".into())
}

/// One server's folder, opened so no path can leave it or pass through a link.
pub struct Folder {
    dir: Dir,
    root: PathBuf,
}

/// A link where Core will not follow one.
fn link(path: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{path} goes through a link, which Core does not follow"),
    )
}

/// Open the folder `name` inside `dir`, refusing a link. `path` names it in errors.
pub fn open_child(dir: &Dir, name: &str, path: &str) -> io::Result<Dir> {
    if dir.symlink_metadata(name)?.file_type().is_symlink() {
        return Err(link(path));
    }
    dir.open_dir_nofollow(name)
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => error,
            _ => link(path),
        })
}

/// Open the plain file `name` inside `dir` without following a link or blocking, and check the
/// handle is a plain file before anything reads it. `path` names it in errors.
pub fn open_plain(
    dir: &Dir,
    name: &str,
    path: &str,
) -> io::Result<cap_std::fs::File> {
    let not_plain = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{path} is not a plain file"),
        )
    };
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true);
    cap_fs_ext::OpenOptionsFollowExt::follow(
        &mut options,
        cap_fs_ext::FollowSymlinks::No,
    );
    cap_fs_ext::OpenOptionsSyncExt::nonblock(&mut options, true);
    let file = match dir.open_with(name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(error);
        }
        // Java writes some files, such as `level.dat`, that only `copal-servers` may read. Say
        // so, instead of calling them something other than a plain file.
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("Core may not read {path}"),
            ));
        }
        Err(error) if dir.symlink_metadata(name).is_ok() => {
            let _ = error;
            return Err(not_plain());
        }
        Err(error) => return Err(error),
    };
    if !file.metadata()?.is_file() {
        return Err(not_plain());
    }
    Ok(file)
}

impl Folder {
    /// Open a server folder. The folder itself must not be a link.
    pub fn open(root: &Path) -> io::Result<Self> {
        if std::fs::symlink_metadata(root)?.file_type().is_symlink() {
            return Err(link(&root.display().to_string()));
        }
        Ok(Self {
            dir: Dir::open_ambient_dir(root, ambient_authority())?,
            root: root.to_owned(),
        })
    }

    /// Whether this folder's file system treats names that differ only in case as one, found by
    /// making a file and looking for it under another case.
    pub fn case_insensitive(&self) -> io::Result<bool> {
        let probe = format!(".copal-case-{}", uuid::Uuid::new_v4().simple());
        self.dir.create(&probe)?;
        let found = self.dir.symlink_metadata(probe.to_uppercase()).is_ok();
        self.dir.remove_file(&probe)?;
        Ok(found)
    }

    /// The world folder this server uses.
    pub fn level_name(&self) -> String {
        level_name(&self.root)
    }

    /// The folder at `path`, or the server folder itself for `None`.
    pub fn open_dir(&self, path: Option<&str>) -> io::Result<Dir> {
        let mut dir = self.dir.try_clone()?;
        for part in path.into_iter().flat_map(|path| path.split('/')) {
            dir = open_child(&dir, part, path.unwrap_or_default())?;
        }
        Ok(dir)
    }

    /// The folder holding `path`, and the last part of `path`.
    fn parent<'a>(&self, path: &'a str) -> io::Result<(Dir, &'a str)> {
        match path.rsplit_once('/') {
            Some((parent, name)) => Ok((self.open_dir(Some(parent))?, name)),
            None => Ok((self.dir.try_clone()?, path)),
        }
    }

    /// As [`Folder::parent`], creating the folders above `path` that are missing.
    fn parent_creating<'a>(&self, path: &'a str) -> io::Result<(Dir, &'a str)> {
        match path.rsplit_once('/') {
            Some((parent, name)) => Ok((self.create_dir_all(parent)?, name)),
            None => Ok((self.dir.try_clone()?, path)),
        }
    }

    /// Create `path` and the folders above it, writable by servers, and open it.
    pub fn create_dir_all(&self, path: &str) -> io::Result<Dir> {
        let mut dir = self.dir.try_clone()?;
        for part in path.split('/') {
            match dir.create_dir(part) {
                Ok(()) => shared(&dir, Path::new(part), true)?,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            dir = open_child(&dir, part, path)?;
        }
        Ok(dir)
    }

    /// Write `path` whole: into a file beside it, renamed over it once complete. A server that
    /// has the old file open keeps reading the old one.
    pub fn write(&self, path: &str, reader: &mut impl Read) -> io::Result<u64> {
        let (dir, name) = self.parent_creating(path)?;
        let staged = staging();
        let result = (|| {
            let mut file = dir.create(&staged)?;
            let written = io::copy(reader, &mut file)?;
            file.flush()?;
            drop(file);
            shared(&dir, Path::new(&staged), false)?;
            dir.rename(&staged, &dir, name)?;
            Ok(written)
        })();
        if result.is_err() {
            let _ = dir.remove_file(&staged);
        }
        result
    }

    /// The plain file at `path`, opened without following a link or blocking. `None` when
    /// nothing is there; an error for a link, a folder, a FIFO, or anything else.
    pub fn open_file(
        &self,
        path: &str,
    ) -> io::Result<Option<cap_std::fs::File>> {
        let (dir, name) = match self.parent(path) {
            Ok(parent) => parent,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        match open_plain(&dir, name, path) {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The SHA-1 of the file at `path`, or `None` when there is no plain file there.
    pub fn sha1(&self, path: &str) -> io::Result<Option<String>> {
        match self.open_file(path) {
            Ok(Some(file)) => sha1_of(file.into_std()).map(Some),
            Ok(None) => Ok(None),
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// What is at `path`, without following a link there.
    pub fn metadata(&self, path: &str) -> io::Result<cap_std::fs::Metadata> {
        let (dir, name) = self.parent(path)?;
        dir.symlink_metadata(name)
    }

    /// Whether anything is at `path`, without following a link there.
    pub fn exists(&self, path: &str) -> bool {
        self.metadata(path).is_ok()
    }

    /// Remove the file, link, or folder at `path`. Nothing there is not an error.
    pub fn remove(&self, path: &str) -> io::Result<()> {
        let result = self.parent(path).and_then(|(dir, name)| {
            match dir.symlink_metadata(name) {
                Ok(meta) if meta.is_dir() => dir.remove_dir_all(name),
                Ok(_) => dir.remove_file(name),
                Err(error) => Err(error),
            }
        });
        match result {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }

    /// Remove the folder at `path` only if it is empty.
    pub fn remove_empty_dir(&self, path: &str) -> io::Result<()> {
        let (dir, name) = self.parent(path)?;
        dir.remove_dir(name)
    }

    /// Move `from` to `to`, creating the folders above `to`.
    pub fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        self.move_to(from, self, to)
    }

    /// Move `from` in this folder to `to` in `other`, creating the folders above `to`. Both must
    /// be on one disk: it is a rename.
    pub fn move_to(
        &self,
        from: &str,
        other: &Folder,
        to: &str,
    ) -> io::Result<()> {
        let (from_dir, from_name) = self.parent(from)?;
        let (to_dir, to_name) = other.parent_creating(to)?;
        from_dir.rename(from_name, &to_dir, to_name)
    }
}

/// A unique name beside a file for writing it whole. Short whatever the file is called, so a name
/// near the 255-byte limit can still be written.
fn staging() -> String {
    format!(".{}.part", uuid::Uuid::new_v4().simple())
}

/// Unix: Core's umask keeps what it creates from its group, but servers must be able to change
/// what Core places, as they would their own files. Windows inherits the folder's permissions.
#[cfg_attr(windows, allow(unused_variables))]
fn shared(dir: &Dir, path: &Path, folder: bool) -> io::Result<()> {
    #[cfg(unix)]
    {
        use cap_std::fs::{Permissions, PermissionsExt};
        dir.set_permissions(
            path,
            Permissions::from_mode(if folder { 0o2770 } else { 0o660 }),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_that_leave_or_change_meaning_are_refused() {
        assert_eq!(clean("mods/a.jar").unwrap(), "mods/a.jar");
        // Four bytes, but not four letters: never sliced inside a character.
        for name in [
            "aa\u{e9}.jar",
            "a\u{20ac}",
            "a\u{6a21}",
            "\u{6a21}\u{7ec4}.jar",
        ] {
            assert_eq!(clean(name).unwrap(), name);
        }
        assert_eq!(
            clean("config/sub dir/b.toml").unwrap(),
            "config/sub dir/b.toml"
        );
        for escapes in [
            "../db/copal.db",
            "mods/../../x",
            "/etc/passwd",
            "C:/x",
            "a:stream",
        ] {
            assert!(
                matches!(clean(escapes), Err(PathError::Escapes(_))),
                "{escapes}"
            );
        }
        for invalid in [
            "",
            "mods//a.jar",
            "./a",
            "mods\\..\\a",
            "server.properties.",
            "eula.txt ",
            "NUL",
            "con.txt",
            "lpt1",
            "a\u{0}b",
        ] {
            assert!(
                matches!(clean(invalid), Err(PathError::Invalid(_))),
                "{invalid:?}"
            );
        }
        assert_eq!(from_api("/").unwrap(), None);
        assert_eq!(from_api("/mods/").unwrap().as_deref(), Some("mods"));
    }

    #[test]
    fn server_owned_files_and_worlds_are_denied() {
        for owned in [
            "server.properties",
            "Server.Properties",
            "ops.json",
            "whitelist.json",
            "banned-players.json",
            "banned-ips.json",
            "eula.txt",
            "world",
            "world/level.dat",
            "world_nether/DIM-1/region/r.0.0.mca",
            "world_the_end/level.dat",
        ] {
            assert!(is_server_owned(owned, "world"), "{owned}");
        }
        for content in [
            "mods/a.jar",
            "config/server.properties",
            "worldedit/config.yml",
            "world2/level.dat",
            "plugins/ops.json",
        ] {
            assert!(!is_server_owned(content, "world"), "{content}");
        }
        assert!(is_server_owned("Survival/level.dat", "survival"));
        // Folded the way Windows and macOS fold names, not only ASCII.
        assert!(is_server_owned("w\u{f6}rld/level.dat", "W\u{d6}RLD"));
        assert!(is_server_owned("SERVER.PROPERTIES", "world"));
        assert_ne!(
            fold_case("config/\u{130}.toml"),
            fold_case("config/i\u{307}.toml")
        );
        assert_eq!(fold_case("stra\u{df}e"), "STRA\u{df}E");
        assert!(!is_server_owned("world/level.dat", "survival"));
    }

    #[test]
    fn the_level_name_comes_from_server_properties() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(level_name(dir.path()), "world");
        std::fs::write(
            dir.path().join("server.properties"),
            "motd=x\nlevel-name=Survival\n",
        )
        .unwrap();
        assert_eq!(level_name(dir.path()), "Survival");
        std::fs::write(
            dir.path().join("server.properties"),
            "level-name=../../db\n",
        )
        .unwrap();
        assert_eq!(level_name(dir.path()), "world");
    }

    #[test]
    fn files_are_written_whole_inside_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        folder.write("config/a/b.toml", &mut &b"x = 1"[..]).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config/a/b.toml"))
                .unwrap(),
            "x = 1"
        );
        assert_eq!(
            folder.sha1("config/a/b.toml").unwrap().unwrap(),
            "34bce5f775de97f557a34088509c8bfe1ea17e52"
        );
        assert_eq!(folder.sha1("missing.jar").unwrap(), None);
        assert_eq!(
            std::fs::read_dir(dir.path().join("config/a"))
                .unwrap()
                .count(),
            1
        );
        folder.remove("config").unwrap();
        assert!(!folder.exists("config"));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_core_may_not_read_says_so() {
        use std::os::unix::fs::PermissionsExt;
        // Root reads everything, so there is nothing to check.
        if nix::unistd::geteuid().is_root() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("world")).unwrap();
        let level = dir.path().join("world/level.dat");
        std::fs::write(&level, "").unwrap();
        std::fs::set_permissions(&level, std::fs::Permissions::from_mode(0))
            .unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        let error = folder.open_file("world/level.dat").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("may not read"));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_a_mod_planted_cannot_reach_outside() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("copal.db"), "secret").unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("mods"))
            .unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        // Read as no file at all: the link is never followed.
        assert_eq!(folder.sha1("mods/copal.db").unwrap(), None);
        assert!(folder.open_file("mods/copal.db").is_err());
        assert!(
            folder
                .write("mods/copal.db", &mut &b"overwritten"[..])
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(outside.path().join("copal.db")).unwrap(),
            "secret"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_from_one_place_in_the_folder_to_another_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("world")).unwrap();
        std::fs::write(dir.path().join("world/level.dat"), "world").unwrap();
        // A mod points `mods` at the world, so `mods/level.dat` is `world/level.dat`.
        std::os::unix::fs::symlink("world", dir.path().join("mods")).unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        assert!(
            folder
                .write("mods/level.dat", &mut &b"replaced"[..])
                .is_err()
        );
        assert!(folder.open_dir(Some("mods")).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("world/level.dat"))
                .unwrap(),
            "world"
        );
        // The folder itself may not be a link either.
        let outer = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(dir.path(), outer.path().join("server"))
            .unwrap();
        assert!(Folder::open(&outer.path().join("server")).is_err());
    }

    #[test]
    fn a_name_near_the_length_limit_is_written_whole() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        let name = format!("world/{}.dat", "r".repeat(236));
        folder.write(&name, &mut &b"region"[..]).unwrap();
        assert_eq!(std::fs::read(dir.path().join(&name)).unwrap(), b"region");
    }

    #[test]
    fn case_insensitivity_is_found_on_the_disk_itself() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        let insensitive = folder.case_insensitive().unwrap();
        if cfg!(windows) || cfg!(target_os = "linux") {
            assert_eq!(insensitive, cfg!(windows));
        }
        // The probe leaves nothing behind.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_a_mod_planted_is_refused_without_waiting() {
        let dir = tempfile::tempdir().unwrap();
        nix::unistd::mkfifo(
            &dir.path().join("config.toml"),
            nix::sys::stat::Mode::from_bits_truncate(0o600),
        )
        .unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        assert_eq!(folder.sha1("config.toml").unwrap(), None);
        assert!(folder.open_file("config.toml").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_from_one_place_in_the_folder_to_another_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("world")).unwrap();
        std::fs::write(dir.path().join("world").join("level.dat"), "world")
            .unwrap();
        let status = std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(dir.path().join("mods"))
            .arg(dir.path().join("world"))
            .output()
            .unwrap();
        assert!(status.status.success(), "{status:?}");
        let folder = Folder::open(dir.path()).unwrap();
        assert!(
            folder
                .write("mods/level.dat", &mut &b"replaced"[..])
                .is_err()
        );
        assert!(folder.open_dir(Some("mods")).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("world").join("level.dat"))
                .unwrap(),
            "world"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_a_mod_planted_cannot_reach_outside() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("copal.db"), "secret").unwrap();
        let dir = tempfile::tempdir().unwrap();
        // Any account can make a junction where it may write; no privilege needed.
        let status = std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(dir.path().join("mods"))
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(status.status.success(), "{status:?}");
        let folder = Folder::open(dir.path()).unwrap();
        // Read as no file at all: the link is never followed.
        assert_eq!(folder.sha1("mods/copal.db").unwrap(), None);
        assert!(folder.open_file("mods/copal.db").is_err());
        assert!(
            folder
                .write("mods/copal.db", &mut &b"overwritten"[..])
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(outside.path().join("copal.db")).unwrap(),
            "secret"
        );
    }
}
