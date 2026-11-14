use std::{collections::HashMap, path::PathBuf};

use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::endpoints;
use crate::state::JavaVersion;

endpoints! {
    /// The Java chosen for each major version, keyed by that version.
    GetJavaVersions: Get "/java/versions", (), (), Empty => Json<HashMap<u32, JavaVersion>>;
    /// Chooses the Java for its major version.
    SetJavaVersion: Put "/java/versions", (), (), Json<JavaVersion> => Empty;
    /// The Java installs found on this machine, of one major version when it is given.
    FindJavaInstalls: Get "/java/installs", (), JavaInstallsQuery, Empty => Json<Vec<JavaVersion>>;
    /// Downloads the Java of this major version, and answers the path of its binary.
    InstallJava: Post "/java/installs", (), (), Json<u32> => Json<PathBuf>;
    /// This machine's memory, in KiB.
    GetSystemMemory: Get "/system/memory", (), (), Empty => Json<u64>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct JavaInstallsQuery {
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub version: Option<u32>,
}
