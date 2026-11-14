use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceFileStorage {
    pub file_id: String,
    pub blob_sha512: String,
    pub storage_kind: FileStorageKind,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, sqlx::Type,
)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum FileStorageKind {
    Reflink,
    Hardlink,
    Copy,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, sqlx::Type,
)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum StoredFileStatus {
    Ready,
    Quarantined,
    Deleting,
}
