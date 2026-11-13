use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::{endpoints, instances::InstanceRef};
use crate::logs::{CensoredString, LogType, Logs};

endpoints! {
    /// The instance's log files and crash reports, newest first.
    ListInstanceLogs: Get "/instances/{id}/logs", InstanceRef, LogListing, Empty => Json<Vec<Logs>>;
    GetInstanceLogOutput: Get "/instances/{id}/logs/{log_type}/{filename}", LogFileRef, (), Empty => Json<CensoredString>;
    DeleteInstanceLog: Delete "/instances/{id}/logs/{log_type}/{filename}", LogFileRef, (), Empty => Empty;
    /// The running game's buffered output. Later lines arrive as `Log` events.
    GetLiveLog: Get "/instances/{id}/live-log", InstanceRef, (), Empty => Json<CensoredString>;
    ClearLiveLog: Delete "/instances/{id}/live-log", InstanceRef, (), Empty => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LogListing {
    /// Leaves out each log's output.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub clear_contents: Option<bool>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LogFileRef {
    pub id: String,
    pub log_type: LogType,
    pub filename: String,
}
