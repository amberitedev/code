//! A server's backups and the operations queued on them, in Modrinth Hosting's backup queue shapes
//! (`Archon.BackupsQueue.v1`). A server has [`BackupOptions::limit`] slots: `split_automatic` for
//! scheduled backups, the rest for manual ones.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct BackupsQueue {
    /// Operations queued or running, oldest first.
    pub active_operations: Vec<ActiveOperation>,
    /// Newest first.
    pub backups: Vec<Backup>,
    pub options: BackupOptions,
    /// Null while scheduled backups are off.
    pub next_auto_backup_at: Option<DateTime<Utc>>,
    /// Manual backups that hold a slot. None can be made once this reaches `split_manual`.
    pub manual_count: u32,
    /// Scheduled backups that hold a slot. The oldest go once this passes `split_automatic`.
    pub auto_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ActiveOperation {
    pub backup_id: String,
    pub operation_type: OperationType,
    pub operation_id: i64,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    /// Who started it. Null for the schedule and the Core's own backups.
    pub user_info: Option<UserInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Backup {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub status: BackupStatus,
    /// Made by the schedule, or before a pack update or a restore.
    pub automated: bool,
    /// Set on scheduled backups that hold a slot: when later scheduled backups push it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub rotates_at: Option<DateTime<Utc>>,
    /// Its operations, newest first.
    pub history: Vec<BackupOperation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum BackupStatus {
    Pending,
    InProgress,
    Error,
    Done,
}

/// Making a backup or restoring it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct BackupOperation {
    pub operation_type: OperationType,
    pub operation_id: i64,
    pub state: OperationState,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    /// It ended and nobody dismissed it yet.
    pub should_prompt: bool,
    pub user_info: Option<UserInfo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Create,
    Restore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Pending,
    Ongoing,
    Completed,
    Cancelled,
    Failed,
}

/// A user as their last ticket named them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct BackupOptions {
    pub split_automatic: u32,
    pub split_manual: u32,
    pub limit: u32,
    /// Hours between scheduled backups.
    pub interval: u32,
}

/// What is given changes; the rest stays. The two splits add up to the slots.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PatchBackupOptions {
    /// 0 turns scheduled backups off.
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub split_automatic: Option<u32>,
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub split_manual: Option<u32>,
    /// Hourly at most, every 744 hours at least.
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub interval: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct RotationAck {
    /// Keeping fewer scheduled backups than there are deletes the oldest. Without this the change
    /// is only described; with it, it is the owner's dangerous action.
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub ar: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PatchBackupOptionsResponse {
    pub applied: bool,
    /// The scheduled backups the change deletes, or deleted once applied.
    pub backup_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct BackupRequest {
    /// 1 to 64 characters.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PostBackupResponse {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PatchBackup {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct DeleteManyBackupRequest {
    pub backup_ids: Vec<String>,
}

#[cfg(test)]
pub(super) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![
        BackupsQueue::decl(&cfg),
        ActiveOperation::decl(&cfg),
        Backup::decl(&cfg),
        BackupStatus::decl(&cfg),
        BackupOperation::decl(&cfg),
        OperationType::decl(&cfg),
        OperationState::decl(&cfg),
        UserInfo::decl(&cfg),
        BackupOptions::decl(&cfg),
        PatchBackupOptions::decl(&cfg),
        RotationAck::decl(&cfg),
        PatchBackupOptionsResponse::decl(&cfg),
        BackupRequest::decl(&cfg),
        PostBackupResponse::decl(&cfg),
        PatchBackup::decl(&cfg),
        DeleteManyBackupRequest::decl(&cfg),
    ]
}
