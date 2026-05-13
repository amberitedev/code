use serde::{Deserialize, Serialize};

use super::instance::{InstanceId, InstanceStatus};

/// Broadcast events emitted by instances and the macro engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
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
	FsChanged {
		instance_id: InstanceId,
		operation: FsOperationKind,
		path: String,
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
