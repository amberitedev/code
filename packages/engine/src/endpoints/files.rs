use chrono::{DateTime, Utc};
use contracts::{Empty, Json, Raw, Stream};
use serde::{Deserialize, Serialize};

use super::{endpoints, instances::InstanceRef};
use crate::{instance::InstanceFileItem, state::IconConfig};

endpoints! {
    /// A folder's entries; `path` is `""` for the instance's own folder.
    ListInstanceFiles: Get "/instances/{id}/files", InstanceRef, InstanceFilePath, Empty => Json<Vec<InstanceFileItem>>;
    ReadInstanceFile: Get "/instances/{id}/files/content", InstanceRef, InstanceFilePath, Empty => Raw;
    WriteInstanceFile: Put "/instances/{id}/files/content", InstanceRef, InstanceFileWrite, Raw => Empty;
    CreateInstanceDirectory: Post "/instances/{id}/files/directories", InstanceRef, InstanceFilePath, Empty => Empty;
    /// Renames or moves; refused when something is already at `destination`.
    MoveInstanceFile: Post "/instances/{id}/files/move", InstanceRef, (), Json<InstanceFileMove> => Empty;
    DeleteInstanceFile: Delete "/instances/{id}/files", InstanceRef, InstanceFileDelete, Empty => Empty;
    /// Extracts a zip into `target`, by default its own folder. A dry run extracts nothing and
    /// answers which files it would replace; a real run answers `null`.
    ExtractInstanceZip: Post "/instances/{id}/files/extract", InstanceRef, (), Json<InstanceZipExtract> => Json<Option<ExtractDryRunResult>>;
    /// What is at `path`, without following a link there.
    StatInstanceFile: Get "/instances/{id}/files/stat", InstanceRef, InstanceFilePath, Empty => Json<InstanceFileStat>;
    /// Writes a zip of a folder, or of some paths in one folder, skipping links.
    ZipInstanceFiles: Post "/instances/{id}/files/zip", InstanceRef, (), Json<InstanceZipRequest> => Empty;
    /// Opens an upload for the caller, ending any open one they had in its scope. Files wait
    /// outside the instance until it is finalized.
    OpenInstanceUpload: Post "/instances/{id}/uploads", InstanceRef, (), Json<InstanceUploadScope> => Json<InstanceUpload>;
    /// The caller's open upload in a scope, if any.
    GetInstanceUpload: Get "/instances/{id}/uploads", InstanceRef, InstanceUploadScope, Empty => Json<Option<InstanceUpload>>;
    /// Adds the body as a file: `path` is where it goes for `files`, its file name for `content`.
    AddInstanceUploadFile: Post "/instances/{id}/uploads/{upload}/files", InstanceUploadRef, InstanceFilePath, Stream => Json<InstanceUpload>;
    /// Puts every file in place, never over a server-owned file; content goes through the
    /// instance's content.
    FinalizeInstanceUpload: Post "/instances/{id}/uploads/{upload}/finalize", InstanceUploadRef, (), Empty => Json<InstanceUpload>;
    CancelInstanceUpload: Delete "/instances/{id}/uploads/{upload}", InstanceUploadRef, (), Empty => Json<InstanceUpload>;
    /// The instance's icon as a PNG, up to 512x512.
    GetInstanceIcon: Get "/instances/{id}/icon", InstanceRef, (), Empty => Raw;
    /// The body is a PNG, JPEG, GIF or WebP image. Clears the icon editor's settings.
    SetInstanceIcon: Put "/instances/{id}/icon", InstanceRef, (), Raw => Empty;
    /// An icon made in the icon editor, kept with the settings it was made from.
    SetEditedInstanceIcon: Put "/instances/{id}/icon/edited", InstanceRef, (), Json<EditedInstanceIcon> => Empty;
    RemoveInstanceIcon: Delete "/instances/{id}/icon", InstanceRef, (), Empty => Empty;
}

/// A path inside the instance's folder, `/`-separated.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceFilePath {
    pub path: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceFileWrite {
    pub path: String,
    /// Refuses to replace a file already there.
    pub create_only: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceFileMove {
    pub source: String,
    pub destination: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceFileDelete {
    pub path: String,
    /// Deletes a folder that is not empty.
    pub recursive: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceZipExtract {
    pub path: String,
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub target: Option<String>,
    pub override_conflicts: bool,
    pub dry_run: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ExtractDryRunResult {
    pub modpack_name: Option<String>,
    pub conflicting_files: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceFileStat {
    pub name: String,
    pub path: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub kind: InstanceFileKind,
    pub modified: DateTime<Utc>,
    pub created: DateTime<Utc>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum InstanceFileKind {
    Regular,
    Directory,
    Symlink,
    Other,
}

/// A folder, zipped into `<path>.zip`, or `include`'s names inside `parent`, zipped into
/// `target`.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "target_type")]
pub enum InstanceZipRequest {
    Directory {
        path: String,
    },
    ManyPaths {
        parent: String,
        include: Vec<String>,
        target: String,
    },
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceUploadRef {
    pub id: String,
    pub upload: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceUploadScope {
    pub scope: UploadScope,
}

/// Content goes through the instance's content; files land where they are put.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum UploadScope {
    Content,
    Files,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum InstanceUploadStatus {
    Active,
    Uploading,
    Finalizing,
    Cancelled,
    Finalized,
}

/// Times are Unix seconds.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceUpload {
    pub upload_id: String,
    pub status: InstanceUploadStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_upload_at: Option<i64>,
    pub expires_at: i64,
    pub entry_count: u32,
    pub uploaded_byte_count: u64,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct EditedInstanceIcon {
    pub config: IconConfig,
    /// The icon the editor drew, as an image.
    pub image: Vec<u8>,
}
