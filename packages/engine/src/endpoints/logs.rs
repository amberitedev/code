use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::instances::InstanceRef;
use crate::logs::{CensoredString, LogType, Logs};

contracts::endpoints! {
    group logs;

    /// The instance's log files and crash reports, newest first.
    ListInstanceLogs: Get "/instances/{id}/logs", Session, InstanceRef, LogListing, Empty => Json<Vec<Logs>>, shared;
    GetInstanceLogOutput: Get "/instances/{id}/logs/{log_type}/{filename}", Session, LogFileRef, (), Empty => Json<CensoredString>, shared;
    DeleteInstanceLog: Delete "/instances/{id}/logs/{log_type}/{filename}", Session, LogFileRef, (), Empty => Empty, shared;
    /// The running game's buffered output. Later lines arrive as `Log` events.
    GetLiveLog: Get "/instances/{id}/live-log", Session, InstanceRef, (), Empty => Json<CensoredString>, client_only;
    ClearLiveLog: Delete "/instances/{id}/live-log", Session, InstanceRef, (), Empty => Empty, client_only;
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
