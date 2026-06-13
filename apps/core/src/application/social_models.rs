use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::infrastructure::minecraft::mrpack::MrpackError;

#[derive(Debug, Error)]
pub enum SocialError {
    #[error("not found")]
    NotFound,
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Mrpack(#[from] MrpackError),
}

#[derive(Debug, Deserialize)]
pub struct UpdateCoreMetadataRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub banner: Option<String>,
    pub subdomain: Option<String>,
    pub setup_mode: Option<String>,
    pub run_mode: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CoreMetadata {
    pub core_id: String,
    pub name: String,
    pub description: Option<String>,
    pub banner: Option<String>,
    pub subdomain: Option<String>,
    pub setup_mode: String,
    pub run_mode: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpsertMemberRequest {
    pub user_id: String,
    pub display_name: Option<String>,
    pub role: String,
    pub permission_preset: Option<String>,
    pub custom_permissions: Option<Value>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CoreMember {
    pub user_id: String,
    pub display_name: Option<String>,
    pub role: String,
    pub permission_preset: String,
    pub custom_permissions: Option<String>,
    pub status: String,
    pub joined_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpsertAccessRequest {
    pub user_id: String,
    pub display_name: Option<String>,
    pub role: String,
    pub permission_preset: Option<String>,
    pub custom_permissions: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct PatchAccessRequest {
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub permission_preset: Option<String>,
    pub custom_permissions: Option<Value>,
}

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct InstanceMember {
    pub instance_id: String,
    pub user_id: String,
    pub display_name: Option<String>,
    pub role: String,
    pub permission_preset: String,
    pub custom_permissions: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct EffectiveAccessMember {
    pub user_id: String,
    pub display_name: Option<String>,
    pub role: String,
    pub permission_preset: String,
    pub custom_permissions: Option<String>,
    pub status: String,
    pub joined_at: String,
    pub updated_at: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct AccessResponse {
    pub members: Vec<EffectiveAccessMember>,
    pub viewer: EffectiveAccessViewer,
}

#[derive(Debug, Serialize)]
pub struct EffectiveAccessViewer {
    pub user_id: String,
    pub role: String,
    pub permission_preset: String,
    pub permissions: Vec<String>,
    pub can_manage_users: bool,
}

#[derive(Debug, Deserialize)]
pub struct ActivityLogQuery {
    pub instance_id: Option<String>,
    pub actor_user_id: Option<String>,
    pub target_user_id: Option<String>,
    pub action: Option<String>,
    pub min_datetime: Option<String>,
    pub max_datetime: Option<String>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ActivityLogEntry {
    pub id: String,
    pub actor_user_id: String,
    pub action: String,
    pub instance_id: Option<String>,
    pub target_user_id: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ActivityLogResponse {
    pub entries: Vec<ActivityLogEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BanMemberRequest {
    pub user_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CoreGroupBan {
    pub user_id: String,
    pub reason: Option<String>,
    pub banned_by: Option<String>,
    pub banned_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterSyncProfileRequest {
    pub id: Option<String>,
    pub client_profile_id: Option<String>,
    pub core_instance_id: Option<String>,
    pub name: String,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub sync_enabled: Option<bool>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SyncProfile {
    pub id: String,
    pub client_profile_id: Option<String>,
    pub core_instance_id: Option<String>,
    pub name: String,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub sync_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_snapshot_at: Option<String>,
    pub current_snapshot_id: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SyncSnapshot {
    pub id: String,
    pub profile_id: String,
    pub author_user_id: String,
    pub manifest_json: String,
    pub client_only_json: Option<String>,
    pub server_manifest_json: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub archive_path: Option<String>,
    pub archived: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SyncEvent {
    pub id: String,
    pub profile_id: String,
    pub snapshot_id: Option<String>,
    pub status: String,
    pub diff_json: Option<String>,
    pub message: Option<String>,
    pub created_at: String,
    pub applied_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSyncProfileFromMrpackRequest {
    pub name: Option<String>,
    pub client_profile_id: Option<String>,
    pub core_instance_id: Option<String>,
    pub sync_enabled: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncSnapshotPublishResult {
    pub profile: SyncProfile,
    pub snapshot: SyncSnapshot,
    pub event: SyncEvent,
}

#[derive(Debug, Serialize)]
pub struct SyncVersionStatus {
    pub profile_id: String,
    pub current_snapshot_id: Option<String>,
    pub current_snapshot_created_at: Option<String>,
}
