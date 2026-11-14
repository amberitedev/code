use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{endpoints, instances::InstanceRef};
use crate::state::ProcessMetadata;

endpoints! {
    /// The running games of every instance the caller may view. `Process` events report starts and
    /// exits.
    ListProcesses: Get "/processes", (), (), Empty => Json<Vec<ProcessMetadata>>;
    ListInstanceProcesses: Get "/instances/{id}/processes", InstanceRef, (), Empty => Json<Vec<ProcessMetadata>>;
    /// Needs Edit on the game's instance. A game that already exited is not an error.
    KillProcess: Post "/processes/{uuid}/kill", ProcessRef, (), Empty => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ProcessRef {
    pub uuid: Uuid,
}
