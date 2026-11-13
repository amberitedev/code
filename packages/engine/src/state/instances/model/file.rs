use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceFile {
    pub id: String,
    pub instance_id: String,
    pub relative_path: String,
    pub file_name: String,
    pub enabled: bool,
    pub sha1: String,
    pub size: u64,
    pub missing: bool,
    pub pending: Option<PendingChange>,
    pub added_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

/// What a pack update wants for a file on a server that it found edited and left alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PendingChange {
    /// The new pack has another version of the file.
    Changed,
    /// The new pack no longer has the file.
    Removed,
}

impl PendingChange {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "changed" => Some(Self::Changed),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }
}
