//! The data folder. Its layout is the isolation story: Core runs as the `copal` OS account and
//! every server, installer, and custom Java runs as `copal-servers`, which may touch only what
//! [`FOLDERS`] grants it. `copal install` creates the folders with these permissions (see
//! `daemon::folders`); nothing else may put a file Core trusts somewhere `copal-servers` can write.
//!
//! The data folder itself is owned by `copal` and only listable by others. Besides the folders it
//! holds `config.toml` and, on Linux and macOS, `copal.sock`, the local socket.

use std::path::{Path, PathBuf};

/// What the `copal-servers` account may do with a folder. The `copal` account owns every folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServersAccess {
    None,
    ReadOnly,
    ReadWrite,
}

/// One top-level folder of the data folder.
pub struct Folder {
    pub name: &'static str,
    pub servers: ServersAccess,
    #[allow(dead_code)] // Documents the table; nothing reads it.
    pub holds: &'static str,
}

/// Every top-level folder. `runs/` and `servers/` hold one folder per server, which inherits the
/// access of its parent.
pub const FOLDERS: [Folder; 9] = [
    Folder {
        name: "db",
        servers: ServersAccess::None,
        holds: "database, sessions, Core's backend key, TLS key",
    },
    Folder {
        name: "backups",
        servers: ServersAccess::None,
        holds: "every backup",
    },
    Folder {
        name: "versions",
        servers: ServersAccess::None,
        holds: "installed Core versions",
    },
    Folder {
        name: "sockets",
        servers: ServersAccess::None,
        holds: "shim command sockets",
    },
    Folder {
        name: "runs",
        servers: ServersAccess::None,
        holds: "per server: pid, run id, status, console.log",
    },
    Folder {
        name: "logs",
        servers: ServersAccess::None,
        holds: "Core's own rotating log",
    },
    Folder {
        name: "servers",
        servers: ServersAccess::ReadWrite,
        holds: "per server: world, mods, config. The only place a mod can write",
    },
    Folder {
        name: "meta",
        servers: ServersAccess::ReadOnly,
        holds: "server jars, libraries",
    },
    Folder {
        name: "java",
        servers: ServersAccess::ReadOnly,
        holds: "Java runtimes",
    },
];

/// Paths inside one data folder. Build every path through here so the layout stays in one place.
#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The optional settings file. See `daemon::config`.
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("db").join("copal.db")
    }

    /// The self-signed certificate the App pins, and its key.
    pub fn tls_cert(&self) -> PathBuf {
        self.root.join("db").join("tls-cert.der")
    }

    pub fn tls_key(&self) -> PathBuf {
        self.root.join("db").join("tls-key.der")
    }

    #[allow(dead_code)] // Wired in step 5: backups.
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }

    /// Installed Core versions, one folder each. See `update::versions`.
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }

    /// Which version the launcher runs. See `update::versions::State`.
    pub fn version_state(&self) -> PathBuf {
        self.versions().join("state.json")
    }

    /// One installed version's folder. `version` is a parsed semver version, so it is a plain
    /// folder name.
    pub fn version(&self, version: &str) -> PathBuf {
        self.versions().join(version)
    }

    /// The database as it was before the last update migrated it, kept for a rollback.
    pub fn database_before_update(&self) -> PathBuf {
        self.root.join("db").join("before-update")
    }

    pub fn sockets(&self) -> PathBuf {
        self.root.join("sockets")
    }

    /// Holds the lock that keeps a second Core off this data folder.
    pub fn core_lock(&self) -> PathBuf {
        self.root.join("db").join("core.lock")
    }

    /// What `copal install` recorded. See `daemon::install::InstallRecord`.
    pub fn install_record(&self) -> PathBuf {
        self.root.join("db").join("install.toml")
    }

    /// Where the CLI reaches Core: a Unix socket in the data folder, or on Windows a named pipe
    /// whose name comes from the data folder, so Cores on different data folders never collide.
    pub fn local_socket(&self) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(self.pipe_name(""))
        } else {
            self.root.join("copal.sock")
        }
    }

    /// The command socket of one server's shim: in `sockets/` on Linux and macOS, a named pipe on
    /// Windows, where the pipe's own permissions do the job of the folder's.
    pub fn shim_socket(&self, server_id: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(self.pipe_name(&format!("-{server_id}")))
        } else {
            self.sockets().join(format!("{server_id}.sock"))
        }
    }

    fn pipe_name(&self, suffix: &str) -> String {
        use sha2::{Digest, Sha256};
        let root = std::path::absolute(&self.root)
            .unwrap_or_else(|_| self.root.clone());
        let hash =
            Sha256::digest(root.to_string_lossy().to_lowercase().as_bytes());
        format!(r"\\.\pipe\copal-{}{suffix}", hex::encode(&hash[..6]))
    }

    /// Run state the shim writes for one server.
    pub fn run(&self, server_id: &str) -> PathBuf {
        self.root.join("runs").join(server_id)
    }

    /// Held by a server's shim for as long as it runs.
    pub fn run_lock(&self, server_id: &str) -> PathBuf {
        self.run(server_id).join("lock")
    }

    /// What Core asks the shim to start. See `servers::shim::Launch`.
    pub fn launch_spec(&self, server_id: &str) -> PathBuf {
        self.run(server_id).join("launch.json")
    }

    /// Who the running shim is. See `servers::shim::RunRecord`.
    pub fn run_record(&self, server_id: &str) -> PathBuf {
        self.run(server_id).join("run.json")
    }

    /// How the last run ended. See `servers::shim::ExitStatus`.
    pub fn run_status(&self, server_id: &str) -> PathBuf {
        self.run(server_id).join("status.json")
    }

    /// The server's output, one line per line, as the shim saw it.
    pub fn console_log(&self, server_id: &str) -> PathBuf {
        self.run(server_id).join("console.log")
    }

    /// The shim's own messages.
    pub fn shim_log(&self, server_id: &str) -> PathBuf {
        self.run(server_id).join("shim.log")
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// One server's folder: the only place `copal-servers` can write.
    pub fn server(&self, server_id: &str) -> PathBuf {
        self.root.join("servers").join(server_id)
    }

    /// The shared Daedalus store. See `installs::daedalus::SharedDirs`.
    #[allow(dead_code)] // Wired in step 4: installs reach the store through this.
    pub fn meta(&self) -> PathBuf {
        self.root.join("meta")
    }

    /// Managed Java runtimes, one folder per major version.
    pub fn java(&self) -> PathBuf {
        self.root.join("java")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_is_inside_a_listed_folder() {
        let paths = Paths::new("data");
        let used = [
            paths.database(),
            paths.tls_cert(),
            paths.tls_key(),
            paths.backups(),
            paths.versions(),
            paths.version_state(),
            paths.version("1.0.0"),
            paths.database_before_update(),
            paths.sockets(),
            paths.core_lock(),
            paths.install_record(),
            paths.run("a"),
            paths.run_lock("a"),
            paths.launch_spec("a"),
            paths.run_record("a"),
            paths.run_status("a"),
            paths.console_log("a"),
            paths.shim_log("a"),
            paths.logs(),
            paths.server("a"),
            paths.meta(),
            paths.java(),
        ];
        for path in used {
            let top = path.strip_prefix("data").unwrap().components().next();
            let top = top.unwrap().as_os_str().to_str().unwrap();
            assert!(
                FOLDERS.iter().any(|folder| folder.name == top),
                "{} is not in a listed folder",
                path.display()
            );
        }
    }

    #[test]
    fn sockets_differ_per_data_folder_and_server() {
        let one = Paths::new("one");
        let two = Paths::new("two");
        assert_ne!(one.local_socket(), two.local_socket());
        assert_ne!(one.shim_socket("a"), one.shim_socket("b"));
        assert_ne!(one.shim_socket("a"), two.shim_socket("a"));
    }
}
