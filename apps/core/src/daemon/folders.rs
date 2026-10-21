//! Creating the data folder with the owner and permissions `paths::FOLDERS` lists.
//!
//! Linux and macOS: `copal` owns everything. Folders `copal-servers` may use belong to its group
//! and carry the setgid bit, so what Core creates inside them belongs to that group too; `copal`
//! is a member of the group, so it can read what servers write. Windows: each folder gets its own
//! permissions instead of inheriting them from `ProgramData`, and inheritable entries carry them
//! to everything inside.

use color_eyre::eyre::{Result, WrapErr};

use super::{
    accounts::Accounts,
    paths::{FOLDERS, Paths},
};

/// Create every folder that is missing. With `accounts` (`copal install`), also set each
/// folder's owner and permissions; without (development), leave them as the OS makes them.
pub fn create(paths: &Paths, accounts: Option<&Accounts>) -> Result<()> {
    let create = |path: &std::path::Path| {
        std::fs::create_dir_all(path).wrap_err_with(|| {
            format!(
                "could not create {}. Set CORE_DATA_DIR to use another data folder",
                path.display()
            )
        })
    };
    create(paths.root())?;
    for folder in &FOLDERS {
        create(&paths.root().join(folder.name))?;
    }
    if let Some(accounts) = accounts {
        platform::set_root(paths.root(), accounts)?;
        for folder in &FOLDERS {
            let path = paths.root().join(folder.name);
            platform::set(&path, folder.servers, accounts).wrap_err_with(
                || {
                    format!(
                        "could not set the permissions of {}",
                        path.display()
                    )
                },
            )?;
        }
    }
    Ok(())
}

/// Create a folder inside `servers/`, where `copal-servers` may write: a server's folder, or one
/// inside it such as `tmp`. Its parent must exist. A mod can plant a link where the folder should
/// be, so a link is refused rather than followed: Core never chmods or writes through one.
pub fn create_server_folder(path: &std::path::Path) -> std::io::Result<()> {
    match std::fs::create_dir(path) {
        Err(error) if error.kind() != std::io::ErrorKind::AlreadyExists => {
            return Err(error);
        }
        _ => {}
    }
    let refuse = || {
        std::io::Error::other(format!(
            "{} is a link or not a folder, so Core will not use it. Delete it and start again",
            path.display()
        ))
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        // O_NOFOLLOW fails on a link; the mode is then set through the handle, never the path.
        let folder = std::fs::File::options()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_DIRECTORY)
            .open(path)
            .map_err(|_| refuse())?;
        folder.set_permissions(std::fs::Permissions::from_mode(0o2770))?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        let meta = std::fs::symlink_metadata(path)?;
        if !meta.is_dir()
            || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(refuse());
        }
    }
    Ok(())
}

#[cfg(unix)]
mod platform {
    use std::{
        os::unix::fs::{PermissionsExt, chown, lchown},
        path::Path,
    };

    use color_eyre::eyre::Result;

    use crate::daemon::{accounts::Accounts, paths::ServersAccess};

    pub fn set_root(root: &Path, accounts: &Accounts) -> Result<()> {
        chown(root, Some(accounts.core.uid), Some(accounts.core.gid))?;
        // Listable by everyone, so the owner can reach `copal.sock`.
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))?;
        // `config.toml` and the socket; the folders are handled by `set`.
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() && !entry.file_type()?.is_symlink()
            {
                lchown(
                    entry.path(),
                    Some(accounts.core.uid),
                    Some(accounts.core.gid),
                )?;
            }
        }
        Ok(())
    }

    /// Give everything inside `path` to `copal` and `group`. Data kept from an earlier install
    /// belongs to accounts that no longer exist. Links are skipped, never followed; install
    /// refuses to run while a server runs, so nothing swaps entries during the walk.
    ///
    /// With `group_writes` (`servers/`), the group also gets read and write on every entry.
    /// Minecraft writes files such as `level.dat` as 0600, so once `copal` owns them the group
    /// is the only way `copal-servers` can still open its own world.
    fn chown_tree(
        path: &Path,
        uid: u32,
        group: u32,
        group_writes: bool,
    ) -> Result<()> {
        for entry in
            walkdir::WalkDir::new(path).min_depth(1).follow_links(false)
        {
            let entry = entry?;
            if entry.path_is_symlink() {
                continue;
            }
            lchown(entry.path(), Some(uid), Some(group))?;
            if group_writes {
                let mode = entry.metadata()?.permissions().mode() & 0o7777;
                let group_mode = if entry.file_type().is_dir() {
                    0o2070
                } else {
                    0o060
                };
                std::fs::set_permissions(
                    entry.path(),
                    std::fs::Permissions::from_mode(mode | group_mode),
                )?;
            }
        }
        Ok(())
    }

    pub fn set(
        path: &Path,
        servers: ServersAccess,
        accounts: &Accounts,
    ) -> Result<()> {
        let (group, mode) = match servers {
            ServersAccess::None => (accounts.core.gid, 0o700),
            // `servers/` itself is read only; each server's folder inside it is 2770.
            ServersAccess::ReadOnly | ServersAccess::ReadWrite => {
                (accounts.servers.gid, 0o2750)
            }
        };
        chown(path, Some(accounts.core.uid), Some(group))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
        chown_tree(
            path,
            accounts.core.uid,
            group,
            servers == ServersAccess::ReadWrite,
        )
    }

    #[cfg(test)]
    mod tests {
        use std::os::unix::fs::PermissionsExt;

        use super::chown_tree;

        #[test]
        fn a_reinstall_leaves_server_files_open_to_the_group() {
            let dir = tempfile::tempdir().unwrap();
            let world = dir.path().join("world");
            std::fs::create_dir(&world).unwrap();
            let level = world.join("level.dat");
            std::fs::write(&level, "").unwrap();
            std::fs::set_permissions(
                &level,
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
            let mode = |path: &std::path::Path| {
                std::fs::metadata(path).unwrap().permissions().mode() & 0o7777
            };
            let (uid, gid) = (
                nix::unistd::getuid().as_raw(),
                nix::unistd::getgid().as_raw(),
            );
            chown_tree(dir.path(), uid, gid, false).unwrap();
            assert_eq!(mode(&level), 0o600);
            chown_tree(dir.path(), uid, gid, true).unwrap();
            assert_eq!(mode(&level), 0o660);
            assert_eq!(mode(&world) & 0o2070, 0o2070);
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::path::Path;

    use color_eyre::eyre::Result;

    use crate::daemon::{accounts::Accounts, paths::ServersAccess, win};

    /// Read, list, and run, for this folder and everything inside.
    const READ: &str = "0x1200a9";
    /// Read, write, and delete, without changing permissions.
    const MODIFY: &str = "0x1301bf";

    /// SYSTEM, Administrators, and `copal` have full control everywhere.
    fn base(accounts: &Accounts) -> String {
        format!(
            "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;{})",
            accounts.core.sid
        )
    }

    pub fn set_root(root: &Path, accounts: &Accounts) -> Result<()> {
        let sddl =
            format!("{}(A;;{READ};;;{})", base(accounts), accounts.servers.sid);
        Ok(win::set_path_permissions(root, &sddl)?)
    }

    pub fn set(
        path: &Path,
        servers: ServersAccess,
        accounts: &Accounts,
    ) -> Result<()> {
        let sid = &accounts.servers.sid;
        let extra = match servers {
            ServersAccess::None => String::new(),
            ServersAccess::ReadOnly => format!("(A;OICI;{READ};;;{sid})"),
            // List `servers/` itself; modify everything inside each server's folder.
            ServersAccess::ReadWrite => {
                format!("(A;;{READ};;;{sid})(A;OICIIO;{MODIFY};;;{sid})")
            }
        };
        Ok(win::set_path_permissions(
            path,
            &format!("{}{extra}", base(accounts)),
        )?)
    }
}
