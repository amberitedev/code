use std::sync::Arc;

use deno_core::{op2, OpState};
use deno_error::JsErrorBox;

use crate::{
    application::state::AppState,
    domain::instance::{InstanceId, InstanceStatus},
    infrastructure::process::instance_actor::ActorCmd,
};

/// Send a command string to the instance's stdin.
#[op2(fast)]
pub fn op_send_command(
    state: &mut OpState,
    #[string] cmd: String,
) -> Result<(), JsErrorBox> {
    let app = state.borrow::<Arc<AppState>>().clone();
    let id = state.borrow::<InstanceId>().clone();
    if let Some(handle) = app.instances.get(&id) {
        let _ = handle.cmd_tx.try_send(ActorCmd::SendCommand(cmd));
    }
    Ok(())
}

/// Get the current status string of the instance.
#[op2]
#[string]
pub fn op_get_status(state: &mut OpState) -> String {
    let app = state.borrow::<Arc<AppState>>().clone();
    let id = state.borrow::<InstanceId>().clone();
    if app.instances.contains_key(&id) {
        InstanceStatus::Running.to_string()
    } else {
        InstanceStatus::Offline.to_string()
    }
}

/// Request a graceful stop of the instance.
#[op2(fast)]
pub fn op_stop_instance(state: &mut OpState) -> Result<(), JsErrorBox> {
    let app = state.borrow::<Arc<AppState>>().clone();
    let id = state.borrow::<InstanceId>().clone();
    if let Some(handle) = app.instances.get(&id) {
        let _ = handle.cmd_tx.try_send(ActorCmd::GracefulStop);
    }
    Ok(())
}

deno_core::extension!(
    amberite_instance_control,
    ops = [op_send_command, op_get_status, op_stop_instance],
    docs = "Amberite instance control ops"
);
