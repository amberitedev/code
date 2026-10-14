//! A server's files, under `/servers/{id}`: `modrinth/v0/fs/` list, download, create, update, move,
//! delete, copy, and zip; `v1/worlds/{wid}/files/stat` and `.../files/zip`; `v1/fs/unarchive`;
//! `v1/fs/ops/{action}`; and the upload sessions at `v1/worlds/{wid}/{scope}/upload-session`
//! (create, get, files, finalize, cancel). Mirrors `Kyros.Files` and `Kyros.UploadSessions` in
//! `packages/api-client/src/modules/kyros/types.ts`.

#![allow(dead_code)] // Wired in step 4: file routes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Query of GET `fs/download` and PUT `fs/update`. Mirrors the `path` param of
/// `KyrosFilesV0Module.downloadFile` and `.updateFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct PathQuery {
    pub path: String,
}

/// Query of GET `fs/list`. Mirrors the params of `KyrosFilesV0Module.listDirectory`.
#[derive(Debug, Clone, Deserialize)]
pub struct ListQuery {
    pub path: String,
    pub page: u32,
    pub page_size: u32,
}

/// Mirrors the `type` union of Kyros.Files.v0.DirectoryItem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DirectoryItemType {
    File,
    Directory,
    Symlink,
}

/// One entry of a directory listing. Times are Unix seconds. Mirrors Kyros.Files.v0.DirectoryItem.
#[derive(Debug, Clone, Serialize)]
pub struct DirectoryItem {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: DirectoryItemType,
    pub path: String,
    pub modified: i64,
    pub created: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// Response of GET `fs/list`. `current` is the page returned. Mirrors
/// Kyros.Files.v0.DirectoryResponse.
#[derive(Debug, Clone, Serialize)]
pub struct DirectoryResponse {
    pub items: Vec<DirectoryItem>,
    pub total: u64,
    pub current: u32,
}

/// Mirrors the `type` param of `KyrosFilesV0Module.createFileOrFolder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CreateKind {
    File,
    Directory,
}

/// Query of POST `fs/create`. A file's contents are the request body. Mirrors the params of
/// `KyrosFilesV0Module.createFileOrFolder` and `.uploadFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateQuery {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: CreateKind,
}

/// Query of DELETE `fs/delete`. Mirrors the params of `KyrosFilesV0Module.deleteFileOrFolder`.
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteQuery {
    pub path: String,
    pub recursive: bool,
}

/// Body of POST `fs/move`. Mirrors the body of `KyrosFilesV0Module.moveFileOrFolder`.
#[derive(Debug, Clone, Deserialize)]
pub struct MoveRequest {
    pub source: String,
    pub destination: String,
}

/// Body of POST `fs/copy` and `fs/zip`. The api-client has no method for these routes; this is the
/// shape the old Core accepted.
#[derive(Debug, Clone, Deserialize)]
pub struct CopyRequest {
    pub sources: Vec<String>,
    pub destination: String,
}

/// Query of POST `v1/fs/unarchive`: extract `src` into `trg`. `dry` only reports conflicts.
/// Mirrors the params of `KyrosFilesV0Module.extractFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct UnarchiveQuery {
    pub src: String,
    pub trg: String,
    #[serde(rename = "override")]
    pub overwrite: bool,
    pub dry: bool,
}

/// Response of POST `v1/fs/unarchive`. Mirrors Kyros.Files.v0.ExtractResult.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractResult {
    pub modpack_name: Option<String>,
    pub conflicting_files: Vec<String>,
}

/// The `{action}` of POST `v1/fs/ops/{action}`. Mirrors the `action` of
/// `KyrosFilesV0Module.modifyOperation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationAction {
    Dismiss,
    Cancel,
}

/// Query of POST `v1/fs/ops/{action}`. Mirrors the `id` param of
/// `KyrosFilesV0Module.modifyOperation`.
#[derive(Debug, Clone, Deserialize)]
pub struct OperationQuery {
    pub id: String,
}

/// Body of POST `v1/worlds/{wid}/files/stat`. Mirrors Kyros.Files.v1.FileStatRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct FileStatRequest {
    pub path: String,
}

/// Mirrors Kyros.Files.v1.DescendantType.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DescendantType {
    Regular,
    Directory,
    Symlink,
    Other,
}

/// Response of POST `v1/worlds/{wid}/files/stat`. Mirrors Kyros.Files.v1.FileStatResponse.
#[derive(Debug, Clone, Serialize)]
pub struct FileStatResponse {
    pub name: String,
    pub full_path: String,
    pub size_bytes: u64,
    #[serde(rename = "type")]
    pub kind: DescendantType,
    pub mtime: DateTime<Utc>,
    pub ctime: DateTime<Utc>,
}

/// Body of POST `v1/worlds/{wid}/files/zip`: zip a directory, or some paths inside `parent` into
/// `target`. Mirrors Kyros.Files.v1.ZipRequest.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "target_type")]
pub enum ZipRequest {
    Directory {
        path: String,
    },
    ManyPaths {
        parent: String,
        include: Vec<String>,
        target: String,
    },
}

/// One record of the `application/json-seq` response of POST `v1/worlds/{wid}/files/zip`. Mirrors
/// Kyros.Files.v1.ZipProgress.
#[derive(Debug, Clone, Serialize)]
pub struct ZipProgress {
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The `{scope}` of the upload session routes: content goes through the content model, files
/// land where they are put. Mirrors Kyros.UploadSessions.v1.Scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadScope {
    Content,
    Files,
}

/// Mirrors Kyros.UploadSessions.v1.UploadSessionStatus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadSessionStatus {
    Active,
    Uploading,
    Finalizing,
    Cancelled,
    Finalized,
    Expired,
}

/// Response of POST `.../upload-session` (create), POST `.../{upload}/files`, POST
/// `.../{upload}/finalize`, and DELETE `.../{upload}` (cancel). Times are Unix seconds.
/// Mirrors Kyros.UploadSessions.v1.UploadSessionResponse.
#[derive(Debug, Clone, Serialize)]
pub struct UploadSessionResponse {
    pub upload_id: String,
    pub status: UploadSessionStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_upload_at: Option<i64>,
    pub expires_at: i64,
    pub entry_count: u32,
    pub uploaded_byte_count: u64,
}

/// Response of GET `.../upload-session`: the caller's open session, if any. Mirrors
/// Kyros.UploadSessions.v1.GetUploadSessionResponse.
#[derive(Debug, Clone, Serialize)]
pub struct GetUploadSessionResponse {
    pub session: Option<UploadSessionResponse>,
}
