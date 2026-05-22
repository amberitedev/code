use serde::{Deserialize, Serialize};

use super::instance::{
    InstanceId, InstanceInstallStatus, InstanceRecord, InstanceStatus,
};

/// Broadcast events emitted by instances and the macro engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    InstanceCreated {
        instance: InstanceRecord,
    },
    InstanceUpdated {
        instance: InstanceRecord,
    },
    InstanceDeleted {
        instance_id: InstanceId,
    },
    InstanceOutput {
        instance_id: InstanceId,
        line: String,
    },
    StatusChanged {
        instance_id: InstanceId,
        status: InstanceStatus,
    },
    MacroOutput {
        instance_id: InstanceId,
        macro_pid: u64,
        line: String,
    },
    CreationProgress {
        instance_id: InstanceId,
        progress: f32,
        message: String,
    },
    InstallStatusChanged {
        instance_id: InstanceId,
        install_status: InstanceInstallStatus,
        message: Option<String>,
    },
    FsChanged {
        instance_id: InstanceId,
        operation: FsOperationKind,
        path: String,
    },
    SyncProfileUpdated {
        profile_id: String,
        snapshot_id: String,
        instance_id: Option<String>,
    },
    SyncEventStatusChanged {
        profile_id: String,
        event_id: String,
        status: String,
        message: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsOperationKind {
    Write,
    Create,
    Delete,
    Move { from: String },
    Upload,
    Zip,
    Unzip,
    Copy,
}
