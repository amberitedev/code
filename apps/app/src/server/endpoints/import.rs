//! Instances from other launchers' folders on this desktop.

use std::path::PathBuf;

use contracts::{Auth, Empty, Endpoint, Json, Method};
use engine::{install::InstallJobSnapshot, pack::import::ImportLauncherType};
use serde::{Deserialize, Serialize};

/// The names of the instance folders under a launcher's folder.
pub struct ListImportableInstances;

impl Endpoint for ListImportableInstances {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/import/instances";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = LauncherFolder;
    type Body = Empty;
    type Response = Json<Vec<String>>;
}

/// Whether the launcher could have made this instance folder.
pub struct CheckImportableInstance;

impl Endpoint for CheckImportableInstance {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/import/instance";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ImportableInstance;
    type Body = Empty;
    type Response = Json<bool>;
}

/// Where the launcher keeps its instances by default, when that folder exists.
pub struct GetDefaultLauncherPath;

impl Endpoint for GetDefaultLauncherPath {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/import/default-path";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = Launcher;
    type Body = Empty;
    type Response = Json<Option<PathBuf>>;
}

/// Starts copying one instance in as a new one.
pub struct ImportInstance;

impl Endpoint for ImportInstance {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/import";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<InstanceToImport>;
    type Response = Json<ImportJob>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Launcher {
    #[cfg_attr(test, ts(type = "ImportLauncherType"))]
    pub launcher_type: ImportLauncherType,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LauncherFolder {
    #[cfg_attr(test, ts(type = "ImportLauncherType"))]
    pub launcher_type: ImportLauncherType,
    pub base_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ImportableInstance {
    #[cfg_attr(test, ts(type = "ImportLauncherType"))]
    pub launcher_type: ImportLauncherType,
    pub instance_folder: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct InstanceToImport {
    #[cfg_attr(test, ts(type = "ImportLauncherType"))]
    pub launcher_type: ImportLauncherType,
    pub base_path: PathBuf,
    /// The folder's name under `base_path`.
    pub instance_folder: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ImportJob(
    #[cfg_attr(test, ts(type = "InstallJobSnapshot"))] pub InstallJobSnapshot,
);
