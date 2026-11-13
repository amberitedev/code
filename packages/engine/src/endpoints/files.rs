use contracts::{Empty, Json, Raw};
use serde::{Deserialize, Serialize};

use super::{endpoints, instances::InstanceRef};
use crate::instance::InstanceFileItem;

endpoints! {
    /// A folder's entries; `path` is `""` for the instance's own folder.
    ListInstanceFiles: Get "/instances/{id}/files", InstanceRef, InstanceFilePath, Empty => Json<Vec<InstanceFileItem>>;
    ReadInstanceFile: Get "/instances/{id}/files/content", InstanceRef, InstanceFilePath, Empty => Raw;
    WriteInstanceFile: Put "/instances/{id}/files/content", InstanceRef, InstanceFileWrite, Raw => Empty;
    CreateInstanceDirectory: Post "/instances/{id}/files/directories", InstanceRef, InstanceFilePath, Empty => Empty;
    /// Renames or moves; refused when something is already at `destination`.
    MoveInstanceFile: Post "/instances/{id}/files/move", InstanceRef, (), Json<InstanceFileMove> => Empty;
    DeleteInstanceFile: Delete "/instances/{id}/files", InstanceRef, InstanceFileDelete, Empty => Empty;
    /// Extracts a zip into its own folder. A dry run extracts nothing and answers which files it
    /// would replace; a real run answers `null`.
    ExtractInstanceZip: Post "/instances/{id}/files/extract", InstanceRef, (), Json<InstanceZipExtract> => Json<Option<ExtractDryRunResult>>;
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
    pub override_conflicts: bool,
    pub dry_run: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ExtractDryRunResult {
    pub modpack_name: Option<String>,
    pub conflicting_files: Vec<String>,
}
