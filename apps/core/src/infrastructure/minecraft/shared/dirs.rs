//! Directory layout for the global content-addressed shared store.
//!
//! Ported in spirit from app-lib's `state::dirs`. Everything lives under
//! `{data_dir}/meta/` and is shared across ALL installations so that a library
//! downloaded for one build is reused by every other build that needs it:
//!
//! ```text
//! {data_dir}/meta/
//!   libraries/<maven path>/<artifact>.jar   shared, deduped by maven coordinates
//!   versions/<version_id>/<version_id>.json  cached merged version info
//!   versions/<version_id>/server.jar         shared vanilla server jar
//! ```
//!
//! `version_id` is the merged id: `{game_version}` for vanilla, or
//! `{game_version}-{loader_version}` for a modded build.

use std::path::{Path, PathBuf};

/// Paths into the global shared store rooted at `{data_dir}/meta`.
#[derive(Debug, Clone)]
pub struct SharedDirs {
    root: PathBuf,
}

impl SharedDirs {
    /// Build the shared dirs view for a Core data directory.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join("meta"),
        }
    }

    /// `{data_dir}/meta` — the shared store root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `{data_dir}/meta/libraries` — the maven-pathed shared library tree.
    pub fn libraries_dir(&self) -> PathBuf {
        self.root.join("libraries")
    }

    /// `{data_dir}/meta/versions/{version_id}`.
    pub fn version_dir(&self, version_id: &str) -> PathBuf {
        self.root.join("versions").join(version_id)
    }

    /// `{data_dir}/meta/versions/{version_id}/{version_id}.json`.
    pub fn version_info_path(&self, version_id: &str) -> PathBuf {
        self.version_dir(version_id)
            .join(format!("{version_id}.json"))
    }

    /// `{data_dir}/meta/versions/{version_id}/server.jar` — the shared vanilla
    /// server jar for that game version. Loaders launch with this jar on the
    /// classpath rather than copying it per instance.
    pub fn server_jar_path(&self, version_id: &str) -> PathBuf {
        self.version_dir(version_id).join("server.jar")
    }
}
