//! Long operations. Each start takes the caller's `op` id and answers 202 at once; the op reports
//! `AppEvent::Progress` and ends with `AppEvent::OpEnded`, both carrying that id.

use std::{collections::HashMap, path::PathBuf};

use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::IdParams;

contracts::endpoints! {
    group ops;

    /// The ids of the running ops: one missing from it has ended, its `OpEnded` perhaps lost.
    ListOps: Get "/app/ops", None, (), (), Empty => Json<Vec<String>>;
    /// Stops a running op, which ends with `cancelled`.
    CancelOp: Delete "/app/ops/{id}", None, IdParams, (), Empty => Empty;
    /// Zips the App's logs and database to `path`. Stages: preparing, exporting, finishing.
    ExportDebugInfo: Post "/app/ops/debug-export", None, (), (), Json<DebugExport> => Empty;
    /// Downloads `url` to `path`. Stages: waiting, downloading, saving. A status other than a
    /// success saves nothing and ends with it as the `response`.
    SaveExternalFile: Post "/app/ops/external-save", None, (), (), Json<ExternalSave> => Empty;
    /// Uploads a file on this computer straight from disk; it ends with the server's `response`,
    /// whatever its status.
    UploadLocalFile: Post "/app/ops/upload", None, (), (), Json<LocalUpload> => Empty;
    /// Copies a file of an instance to `destination`.
    SaveInstanceFileAs: Post "/app/ops/instance-file-save", None, (), (), Json<InstanceFileSave> => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct DebugExport {
    pub op: String,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ExternalSave {
    pub op: String,
    pub path: PathBuf,
    pub url: String,
    pub headers: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LocalUpload {
    pub op: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub path: PathBuf,
    /// The name a multipart upload sends, the name of `path` by default.
    pub file_name: Option<String>,
    /// Sends the file as this multipart form field rather than as the raw body.
    pub multipart_field: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceFileSave {
    pub op: String,
    pub instance_id: String,
    /// The file's path in the instance.
    pub path: String,
    pub destination: PathBuf,
}

/// What the server an op sent its request to answered.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct OpResponse {
    pub status: u16,
    pub body: String,
}
