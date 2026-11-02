//! A share's local history entry: one push as its owner recorded it, and the same push as the
//! History page shows it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diff::{SharedInstanceUpdateDiff, SharedInstanceUpdateDiffType};
use crate::rules::CONFIG_FILE_TYPE;
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

/// One version of a share's history as the History page shows it: what it runs on, how much it
/// holds, and what it changed over the version before it. The App returns it for an instance's
/// share and Core for a server's, so both pages read one shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SharedInstanceHistoryEntry {
    pub version: i32,
    /// When the push finished, in milliseconds since the Unix epoch.
    pub created: i64,
    pub game_version: String,
    /// The share's loader name, such as `fabric`, or `vanilla` for a server without a client
    /// loader.
    pub loader: String,
    pub loader_version: String,
    /// Projects and files, not counting config files.
    pub content_count: usize,
    pub diffs: Vec<SharedInstanceUpdateDiff>,
    /// Paths of the config files it changed, or of all it holds without a base.
    pub config_files: Vec<String>,
}

impl From<HistoryEntry> for SharedInstanceHistoryEntry {
    fn from(entry: HistoryEntry) -> Self {
        let content_count = entry.content.modrinth_ids.len()
            + entry
                .manifest()
                .filter(|file| file.file_type != CONFIG_FILE_TYPE)
                .count();
        let config_files = entry
            .diffs
            .iter()
            .find(|diff| {
                diff.type_ == SharedInstanceUpdateDiffType::ConfigFilesUpdated
            })
            .map(|diff| diff.config_file_names.clone())
            .unwrap_or_default();
        Self {
            version: entry.version,
            created: entry.created,
            game_version: entry.content.game_version,
            loader: entry.content.loader,
            loader_version: entry.content.loader_version,
            content_count,
            diffs: entry.diffs,
            config_files,
        }
    }
}
