//! Instance routes that take or answer a path on this desktop, so a Core never serves them.

use std::path::PathBuf;

use contracts::{Empty, Json};
use engine::data::ProjectType;
use serde::{Deserialize, Serialize};

use super::IdParams;

endpoints! {
    /// The instance's folder.
    GetInstancePath: Get "/app/instances/{id}/path", IdParams, (), Empty => Json<PathBuf>;
    /// Where a content file of the instance is on disk.
    GetInstanceContentPath: Get "/app/instances/{id}/content-path", IdParams,
        ContentPathQuery, Empty => Json<PathBuf>;
    /// Copies a file on this desktop into the instance. Answers its path in the instance.
    AddProjectFromPath: Post "/app/instances/{id}/projects/from-path", IdParams, (),
        Json<LocalProject> => Json<String>;
    /// Whether Modrinth knows the file by its hash.
    CheckFileOnModrinth: Post "/app/content/on-modrinth", (), (), Json<LocalContentFile> => Json<bool>;
    /// Writes the instance as an `.mrpack` to `location`.
    ExportMrpack: Post "/app/instances/{id}/export-mrpack", IdParams, (),
        Json<MrpackExport> => Empty;
    /// Sets the instance's icon from an image on this desktop, or removes it when `path` is `null`.
    SetInstanceIconFromPath: Put "/app/instances/{id}/icon", IdParams, (),
        Json<LocalIcon> => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ContentPathQuery {
    pub project_path: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LocalProject {
    pub path: PathBuf,
    #[cfg_attr(test, ts(optional = nullable))]
    pub project_type: Option<ProjectType>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LocalContentFile {
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct MrpackExport {
    pub location: PathBuf,
    pub included_overrides: Vec<String>,
    pub excluded_overrides: Vec<String>,
    #[cfg_attr(test, ts(optional = nullable))]
    pub version_id: Option<String>,
    #[cfg_attr(test, ts(optional = nullable))]
    pub description: Option<String>,
    #[cfg_attr(test, ts(optional = nullable))]
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LocalIcon {
    pub path: Option<PathBuf>,
}
