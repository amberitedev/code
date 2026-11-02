//! A share's local history entry: one push as its owner recorded it.

use serde::{Deserialize, Serialize};

use crate::diff::SharedInstanceUpdateDiff;
use crate::versions::{ManifestFile, VersionContent};

/// One push of a share, kept by the App or Core that made it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub version: i32,
    /// When the push finished, in milliseconds since the Unix epoch.
    pub created: i64,
    pub content: VersionContent,
    /// The version's files as its manifest lists them, each with its copy in the store.
    pub files: Vec<HistoryFile>,
    /// What the version changed over the one before it, named when it was pushed.
    pub diffs: Vec<SharedInstanceUpdateDiff>,
}

/// A file of a history entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryFile {
    #[serde(flatten)]
    pub manifest: ManifestFile,
    /// The key of its copy in the store ([`super::HistoryStore::store_file`]).
    pub blob: String,
}

impl HistoryEntry {
    /// The version's manifest files.
    pub fn manifest(&self) -> impl Iterator<Item = &ManifestFile> {
        self.files.iter().map(|file| &file.manifest)
    }
}
