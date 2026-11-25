use std::{collections::HashMap, path::PathBuf};

use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use crate::state::JavaVersion;

contracts::endpoints! {
    group java;

    /// The Java chosen for each major version, keyed by that version.
    GetJavaVersions: Get "/java/versions", Session, (), (), Empty => Json<HashMap<u32, JavaVersion>>, client_only;
    /// Chooses the Java for its major version.
    SetJavaVersion: Put "/java/versions", Session, (), (), Json<JavaVersion> => Empty, client_only;
    /// The Java installs found on this machine, of one major version when it is given.
    FindJavaInstalls: Get "/java/installs", Session, (), JavaInstallsQuery, Empty => Json<Vec<JavaVersion>>, client_only;
    /// Downloads the Java of this major version, and answers the path of its binary.
    InstallJava: Post "/java/installs", Session, (), (), Json<u32> => Json<PathBuf>, client_only;
    /// This machine's memory, in KiB.
    GetSystemMemory: Get "/system/memory", Session, (), (), Empty => Json<u64>, client_only;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct JavaInstallsQuery {
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub version: Option<u32>,
}
