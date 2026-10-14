//! The queued backup engine, under `/v1/servers/{id}/worlds/{wid}/backups-queue`: list, create,
//! `delete-many`, `{bid}` delete, `{bid}/restore`, and `history/{kind}/{operation}/{action}`; and
//! `GET /servers/{id}/modrinth/v0/backups/{bid}/download`. Mirrors `Archon.BackupsQueue.v1` in
//! `packages/api-client/src/modules/archon/types.ts` (`archon/backups-queue/v1.ts`) and
//! `getBackupDownloadUrl` in `utils/node-url.ts`.

#![allow(dead_code)] // Wired in step 5: backup routes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Also the `{kind}` of the history routes. Mirrors
/// Archon.BackupsQueue.v1.BackupQueueOperationType.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupQueueOperationType {
    Create,
    Restore,
}

/// Mirrors Archon.BackupsQueue.v1.BackupQueueState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupQueueState {
    Pending,
    Ongoing,
    Completed,
    Cancelled,
    Failed,
    TimedOut,
}

/// Mirrors Archon.BackupsQueue.v1.BackupStatus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupStatus {
    Pending,
    InProgress,
    TimedOut,
    Error,
    Done,
}

/// Body of POST `backups-queue` (create) and POST `backups-queue/{bid}/restore`. Mirrors
/// Archon.BackupsQueue.v1.BackupRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupRequest {
    pub name: String,
}

/// Response of POST `backups-queue`. Mirrors Archon.BackupsQueue.v1.PostBackupQueueResponse.
#[derive(Debug, Clone, Serialize)]
pub struct PostBackupQueueResponse {
    pub id: Uuid,
}

/// Who started an operation. Mirrors Archon.BackupsQueue.v1.UserInfo.
#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

/// Body of POST `backups-queue/delete-many`. Mirrors
/// Archon.BackupsQueue.v1.DeleteManyBackupRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteManyBackupRequest {
    pub backup_ids: Vec<Uuid>,
}

/// The `{action}` of POST `backups-queue/history/{kind}/{operation}/{action}`: `ack` dismisses a
/// finished operation, `cancel` stops a queued one. Mirrors the `ackCreate`, `cancelCreate`,
/// `ackRestore`, and `cancelRestore` paths of `ArchonBackupsQueueV1Module`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryAction {
    Ack,
    Cancel,
}

/// A queued or running operation. Mirrors Archon.BackupsQueue.v1.ActiveOperation.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveOperation {
    pub backup_id: Uuid,
    pub operation_type: BackupQueueOperationType,
    pub operation_id: Option<i64>,
    pub has_parent: bool,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub synthetic_legacy: bool,
    pub user_info: Option<UserInfo>,
}

/// One past or current operation on a backup. Mirrors
/// Archon.BackupsQueue.v1.BackupQueueOperation.
#[derive(Debug, Clone, Serialize)]
pub struct BackupQueueOperation {
    pub operation_type: BackupQueueOperationType,
    pub operation_id: Option<i64>,
    pub state: BackupQueueState,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub has_parent: bool,
    pub error: Option<String>,
    pub should_prompt: bool,
    pub synthetic_legacy: bool,
    pub user_info: Option<UserInfo>,
}

/// Mirrors Archon.BackupsQueue.v1.BackupQueueBackup.
#[derive(Debug, Clone, Serialize)]
pub struct BackupQueueBackup {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub status: BackupStatus,
    pub locked: bool,
    pub automated: bool,
    pub history: Vec<BackupQueueOperation>,
}

/// Response of GET `backups-queue`. Mirrors Archon.BackupsQueue.v1.BackupsQueueResponse.
#[derive(Debug, Clone, Serialize)]
pub struct BackupsQueueResponse {
    pub active_operations: Vec<ActiveOperation>,
    pub backups: Vec<BackupQueueBackup>,
}

/// Query of GET `/servers/{id}/modrinth/v0/backups/{bid}/download`. A browser follows the link, so
/// the token travels in the query. Mirrors the `auth` param of `getBackupDownloadUrl`.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupDownloadQuery {
    pub auth: String,
}
