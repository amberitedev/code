use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use crate::metadata::Manifest;

contracts::endpoints! {
    group metadata;

    /// The versions of a mod loader for each game version.
    GetLoaderManifest: Get "/metadata/loaders/{loader}", Session, LoaderRef, (), Empty => Json<Manifest>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LoaderRef {
    pub loader: String,
}
