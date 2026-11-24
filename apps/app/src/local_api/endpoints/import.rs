//! Instances from other launchers' folders on this desktop.

use std::path::PathBuf;

use contracts::{Empty, Json};
use engine::{install::InstallJobSnapshot, pack::import::ImportLauncherType};
use serde::{Deserialize, Serialize};

contracts::endpoints! {
    group launcherImport;

    /// The names of the instance folders under a launcher's folder.
    ListImportableInstances: Get "/app/import/instances", None, (), LauncherFolder, Empty => Json<Vec<String>>;
    /// Whether the launcher could have made this instance folder.
    CheckImportableInstance: Get "/app/import/instance", None, (), ImportableInstance, Empty => Json<bool>;
    /// Where the launcher keeps its instances by default, when that folder exists.
    GetDefaultLauncherPath: Get "/app/import/default-path", None, (), Launcher, Empty => Json<Option<PathBuf>>;
    /// Starts copying one instance in as a new one.
    ImportInstance: Post "/app/import", None, (), (), Json<InstanceToImport> => Json<ImportJob>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Launcher {
    #[cfg_attr(feature = "export-ts", ts(type = "ImportLauncherType"))]
    pub launcher_type: ImportLauncherType,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LauncherFolder {
    #[cfg_attr(feature = "export-ts", ts(type = "ImportLauncherType"))]
    pub launcher_type: ImportLauncherType,
    pub base_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ImportableInstance {
    #[cfg_attr(feature = "export-ts", ts(type = "ImportLauncherType"))]
    pub launcher_type: ImportLauncherType,
    pub instance_folder: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceToImport {
    #[cfg_attr(feature = "export-ts", ts(type = "ImportLauncherType"))]
    pub launcher_type: ImportLauncherType,
    pub base_path: PathBuf,
    /// The folder's name under `base_path`.
    pub instance_folder: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ImportJob(
    #[cfg_attr(feature = "export-ts", ts(type = "InstallJobSnapshot"))]
    pub  InstallJobSnapshot,
);
