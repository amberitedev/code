//! App updates. A check keeps what it found, a download keeps the update, and the App installs a
//! downloaded update when it exits.

use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

contracts::endpoints! {
    group updates;

    /// The newer App release, if one exists. `unsupported` in a build without the updater.
    CheckForUpdate: Post "/app/updates/check", None, (), (), Empty => Json<Option<AvailableUpdate>>;
    /// The download size of the update the last check found, when its server says.
    GetUpdateSize: Get "/app/updates/size", None, (), (), Empty => Json<Option<u64>>;
    /// Downloads the update the last check found, reporting on the `launcher_update` loading bar.
    DownloadUpdate: Post "/app/ops/update-download", None, (), (), Json<UpdateDownload> => Empty;
    /// Forgets a downloaded update, so exiting installs nothing.
    DiscardDownloadedUpdate: Delete "/app/updates/downloaded", None, (), (), Empty => Empty;
    /// Whether the App starts again after installing a downloaded update on exit.
    SetRestartAfterUpdate: Put "/app/updates/restart-after", None, (), (), Json<RestartAfterUpdate> => Empty;
    RestartApp: Post "/app/restart", None, (), (), Empty => Empty;
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct AvailableUpdate {
    pub version: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct UpdateDownload {
    pub op: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct RestartAfterUpdate {
    pub restart: bool,
}
