//! Which files a share may carry besides its content set, decided once for the App and Core.
//!
//! A [`ShareScope`] says where the files come from. A file the scope *reaches* is one the owner
//! is shown, by its path in the instance or server folder. It is *blocked* when the share still
//! refuses it for what it is: an executable, by its name or by its leading bytes, or on a server
//! anything but a config file. Both sides list blocked files, so the UI can say why they cannot
//! be picked, and both refuse them in a push.
//!
//! The server's rules are mirrored in `packages/ui/src/composables/server-sharing/config-files.ts`,
//! which takes the extension list from `packages/api-client/src/modules/sharing/rules.ts`,
//! generated from this file. `cargo test -p sharing-client` fails when that file is stale.
//! Regenerate it with `cargo test -p sharing-client -- --ignored write_typescript`.

use std::path::{Component, Path};

use bedrock::state::ProjectType;

/// The manifest type of a single file the owner picked, named by its path.
pub const CONFIG_FILE_TYPE: &str = "config";
/// Leading bytes of each file passed to [`is_executable_content`].
pub const SCAN_HEAD_SIZE: usize = 8192;
/// The folder a server's shared files live in.
pub const SERVER_SHARE_FOLDER: &str = "config";
/// Extensions of the config files a server's share may carry.
pub const CONFIG_EXTENSIONS: [&str; 14] = [
    "json",
    "json5",
    "jsonc",
    "yml",
    "yaml",
    "css",
    "toml",
    "txt",
    "ini",
    "cfg",
    "conf",
    "properties",
    "xml",
    "nbt",
];

/// Extensions of executables, scripts and installers, which no share carries.
const EXECUTABLE_EXTENSIONS: &[&str] = &[
    "exe", "dll", "msi", "msix", "com", "scr", "cpl", "sys", "bat", "cmd",
    "ps1", "psm1", "vbs", "vbe", "jse", "wsf", "wsh", "hta", "lnk", "reg",
    "sh", "bash", "zsh", "command", "app", "dylib", "so", "appimage", "apk",
    "deb", "rpm", "dmg", "pkg", "class",
];

/// Where a share's picked files come from.
#[derive(Clone, Copy, Debug)]
pub enum ShareScope {
    /// An App instance: any path its `.mrpack` export would include, as `exportable` decides
    /// (app-lib's `export_mrpack::is_exportable_path`), outside the content folders, whose
    /// files the content set shares.
    Instance { exportable: fn(&str) -> bool },
    /// A Core's server: config files under `config/`.
    Server,
}

impl ShareScope {
    /// Whether the scope reaches the file at `path`, so the owner is shown it.
    pub fn reaches(self, path: &str) -> bool {
        if !is_safe_relative(path) {
            return false;
        }
        match self {
            Self::Instance { exportable } => {
                exportable(path)
                    && path
                        .split_once('/')
                        .is_none_or(|(root, _)| !is_content_folder(root))
            }
            Self::Server => path.split_once('/').is_some_and(|(root, rest)| {
                root == SERVER_SHARE_FOLDER
                    && !rest.split('/').any(|name| name.starts_with('.'))
            }),
        }
    }

    /// Whether a walk of the folder should enter the directory at `path`.
    pub fn enters(self, path: &str) -> bool {
        match self {
            Self::Instance { exportable } => {
                exportable(path) && !is_content_folder(path)
            }
            Self::Server => {
                path == SERVER_SHARE_FOLDER
                    || path
                        .strip_prefix(SERVER_SHARE_FOLDER)
                        .is_some_and(|rest| rest.starts_with('/'))
            }
        }
    }

    /// Whether a share may carry the file at `path`, judged by its path alone. Files the scope
    /// reaches but does not carry are listed as blocked.
    pub fn carries(self, path: &str) -> bool {
        self.reaches(path)
            && match self {
                Self::Instance { .. } => !is_executable_name(path),
                Self::Server => extension(path).is_some_and(|extension| {
                    CONFIG_EXTENSIONS.contains(&extension.as_str())
                }),
            }
    }

    /// Whether the file at `path`, whose leading bytes (up to [`SCAN_HEAD_SIZE`]) are `head`,
    /// is listed as blocked.
    pub fn blocked(self, path: &str, head: &[u8]) -> bool {
        !self.carries(path) || is_executable_content(head)
    }
}

/// Whether a file's leading bytes are an executable, script or installer, as the shared
/// instances backend checks uploads.
pub fn is_executable_content(head: &[u8]) -> bool {
    infer::get(head).is_some_and(|kind| match kind.matcher_type() {
        infer::MatcherType::App => {
            !matches!(kind.extension(), "der" | "pem" | "obj" | "bc")
        }
        _ => matches!(
            kind.extension(),
            "sh" | "msi" | "cab" | "deb" | "rpm" | "crx"
        ),
    })
}

/// Whether `path` names an executable, script or installer by its extension.
pub fn is_executable_name(path: &str) -> bool {
    extension(path).is_some_and(|extension| {
        EXECUTABLE_EXTENSIONS.contains(&extension.as_str())
    })
}

/// A relative path with `/` separators that stays inside the folder: no empty, `.` or `..`
/// parts, no drive or `\`, and no part ending in a dot or space, which Windows drops. As the
/// content store's `validate_relative`.
pub fn is_safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
        })
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn extension(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    let (_, extension) = name.rsplit_once('.')?;
    Some(extension.to_ascii_lowercase())
}

fn is_content_folder(path: &str) -> bool {
    ProjectType::iterator()
        .any(|project_type| project_type.get_folder() == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exportable(path: &str) -> bool {
        path != "profile.json"
    }

    const INSTANCE: ShareScope = ShareScope::Instance { exportable };

    #[test]
    fn a_server_shares_config_files_only() {
        let server = ShareScope::Server;
        assert!(server.carries("config/a.toml"));
        assert!(server.carries("config/mod/b.JSON"));
        assert!(!server.reaches("config"));
        assert!(!server.reaches("mods/a.toml"));
        assert!(!server.reaches("config/.hidden/a.toml"));
        assert!(!server.reaches("config/a.toml."));
        assert!(server.reaches("config/a.jar"));
        assert!(!server.carries("config/a.jar"));
        assert!(server.enters("config/sub"));
        assert!(!server.enters("configs"));
    }

    #[test]
    fn an_instance_shares_exportable_files_outside_content_folders() {
        assert!(INSTANCE.carries("options.txt"));
        assert!(INSTANCE.carries("config/a.toml"));
        assert!(!INSTANCE.reaches("profile.json"));
        assert!(!INSTANCE.reaches("mods/a.jar"));
        assert!(!INSTANCE.enters("mods"));
        assert!(INSTANCE.reaches("tool.exe"));
        assert!(!INSTANCE.carries("tool.exe"));
        assert!(!INSTANCE.reaches("../a.txt"));
    }

    #[test]
    fn executables_are_blocked_by_their_bytes() {
        assert!(INSTANCE.blocked("options.txt", b"MZ\x90\x00\x03\x00\x00\x00"));
        assert!(!INSTANCE.blocked("options.txt", b"fov:90"));
    }

    mod typescript {
        const PATH: &str = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../api-client/src/modules/sharing/rules.ts"
        );

        fn file() -> String {
            let extensions = super::CONFIG_EXTENSIONS
                .iter()
                .map(|extension| format!("\t'{extension}',\n"))
                .collect::<String>();
            format!(
                "// Generated from packages/sharing-client/src/rules/mod.rs. Do not edit.\n\
                 // Regenerate: cargo test -p sharing-client -- --ignored write_typescript\n\n\
                 /** The folder a server's shared files live in. */\n\
                 export const SERVER_SHARE_FOLDER = '{}'\n\n\
                 /** Extensions of the config files a server's share may carry. */\n\
                 export const SHAREABLE_CONFIG_EXTENSIONS: readonly string[] = [\n{extensions}]\n",
                super::SERVER_SHARE_FOLDER
            )
        }

        #[test]
        fn typescript_is_up_to_date() {
            let written = std::fs::read_to_string(PATH).unwrap_or_default();
            assert!(
                written.replace("\r\n", "\n") == file(),
                "{PATH} is out of date. Run: cargo test -p sharing-client -- --ignored write_typescript"
            );
        }

        #[test]
        #[ignore = "writes the generated file; run on purpose"]
        fn write_typescript() {
            std::fs::create_dir_all(
                std::path::Path::new(PATH).parent().unwrap(),
            )
            .unwrap();
            std::fs::write(PATH, file()).unwrap();
        }
    }
}
