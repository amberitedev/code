//! The data folder. Its layout is the isolation story: Core runs as the `copal` OS account and
//! every server, installer, and custom Java runs as `copal-servers`, which may touch only what
//! [`FOLDERS`] grants it. Step 2 creates the folders with these permissions; nothing else may put
//! a file Core trusts somewhere `copal-servers` can write.

#![allow(dead_code)] // Wired in step 2: `copal install` creates these folders with their permissions.

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

    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }

    pub fn sockets(&self) -> PathBuf {
        self.root.join("sockets")
    }

    /// Run state the shim writes for one server.
    pub fn run(&self, server_id: &str) -> PathBuf {
        self.root.join("runs").join(server_id)
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// One server's folder: the only place `copal-servers` can write.
    pub fn server(&self, server_id: &str) -> PathBuf {
        self.root.join("servers").join(server_id)
    }

    /// The shared Daedalus store. See `installs::daedalus::SharedDirs`.
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
            paths.sockets(),
            paths.run("a"),
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
}
