//! A server's folder, as Core opens it. Mods write in it too and can plant links anywhere inside,
//! so Core opens every path through a `cap_std::fs::Dir` on the folder: a path that would leave it,
//! by `..`, a root, or a link pointing out, fails instead of reaching Core's own files. Paths from
//! the API, packs, and archives are also checked by [`clean`] first, so they fail with a clear
//! reason and mean the same thing on every OS.
//!
//! Files and folders Core creates here stay writable by `copal-servers` (see `daemon::folders`).

use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

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
                || ((stem.len() == 4)
                    && (stem[..3].eq_ignore_ascii_case("COM")
                        || stem[..3].eq_ignore_ascii_case("LPT"))
                    && stem.as_bytes()[3].is_ascii_digit())
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
    let path = path.to_ascii_lowercase();
    if FILES.contains(&path.as_str()) {
        return true;
    }
    let level = level_name.trim_matches('/').to_ascii_lowercase();
    [
        level.clone(),
        format!("{level}_nether"),
        format!("{level}_the_end"),
    ]
    .iter()
    .any(|world| path == *world || path.starts_with(&format!("{world}/")))
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

/// One server's folder, opened so no path can leave it.
pub struct Folder {
    dir: Dir,
    root: PathBuf,
}

impl Folder {
    pub fn open(root: &Path) -> io::Result<Self> {
        Ok(Self {
            dir: Dir::open_ambient_dir(root, ambient_authority())?,
            root: root.to_owned(),
        })
    }

    pub fn dir(&self) -> &Dir {
        &self.dir
    }

    /// The world folder this server uses.
    pub fn level_name(&self) -> String {
        level_name(&self.root)
    }

    /// Create `path` and the folders above it, writable by servers.
    pub fn create_dir_all(&self, path: &str) -> io::Result<()> {
        let mut prefix = PathBuf::new();
        for part in path.split('/') {
            prefix.push(part);
            match self.dir.create_dir(&prefix) {
                Ok(()) => shared(&self.dir, &prefix, true)?,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Write `path` whole: into a file beside it, renamed over it once complete. A server that
    /// has the old file open keeps reading the old one.
    pub fn write(&self, path: &str, reader: &mut impl Read) -> io::Result<u64> {
        if let Some((parent, _)) = path.rsplit_once('/') {
            self.create_dir_all(parent)?;
        }
        let staged = staging(path);
        let result = (|| {
            let mut file = self.dir.create(&staged)?;
            let written = io::copy(reader, &mut file)?;
            file.flush()?;
            drop(file);
            shared(&self.dir, Path::new(&staged), false)?;
            self.dir.rename(&staged, &self.dir, path)?;
            Ok(written)
        })();
        if result.is_err() {
            let _ = self.dir.remove_file(&staged);
        }
        result
    }

    /// The SHA-1 of the file at `path`, or `None` when there is no file there.
    pub fn sha1(&self, path: &str) -> io::Result<Option<String>> {
        match self.dir.open(path) {
            Ok(file) => {
                if !file.metadata()?.is_file() {
                    return Ok(None);
                }
                sha1_of(file.into_std()).map(Some)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Whether anything is at `path`, without following a link there.
    pub fn exists(&self, path: &str) -> bool {
        self.dir.symlink_metadata(path).is_ok()
    }

    /// Remove the file or folder at `path`. Nothing there is not an error.
    pub fn remove(&self, path: &str) -> io::Result<()> {
        let result = match self.dir.symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => self.dir.remove_dir_all(path),
            Ok(_) => self.dir.remove_file(path),
            Err(error) => Err(error),
        };
        match result {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }

    /// Move `from` to `to`, creating the folders above `to`.
    pub fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        if let Some((parent, _)) = to.rsplit_once('/') {
            self.create_dir_all(parent)?;
        }
        self.dir.rename(from, &self.dir, to)
    }
}

/// A unique name beside `path` for writing it whole.
fn staging(path: &str) -> String {
    let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
    let staged = format!(".{name}.{}.part", uuid::Uuid::new_v4());
    if parent.is_empty() {
        staged
    } else {
        format!("{parent}/{staged}")
    }
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
    fn a_link_a_mod_planted_cannot_reach_outside() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("copal.db"), "secret").unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("mods"))
            .unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        assert!(folder.sha1("mods/copal.db").is_err());
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
        assert!(folder.sha1("mods/copal.db").is_err());
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
