use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::{op2, OpState};
use deno_error::JsErrorBox;
use tokio::sync::{broadcast, Mutex};

use crate::domain::{
    event::Event,
    instance::InstanceId,
};

pub type EventRx = Arc<Mutex<broadcast::Receiver<Event>>>;

/// Block until the next stdout line from this macro's instance arrives.
#[op2(async)]
#[string]
pub async fn op_next_instance_output(
    state: Rc<RefCell<OpState>>,
) -> Result<Option<String>, JsErrorBox> {
    let (rx_arc, instance_id) = {
        let s = state.borrow();
        (
            s.borrow::<EventRx>().clone(),
            s.borrow::<InstanceId>().clone(),
        )
    };
    loop {
        let event = {
            let mut guard = rx_arc.lock().await;
            match guard.recv().await {
                Ok(e) => e,
                Err(_) => return Ok(None),
            }
        };
        if let Event::InstanceOutput { instance_id: eid, line } = event {
            if eid == instance_id {
                return Ok(Some(line));
            }
        }
    }
}

/// Block until the next status change for this macro's instance.
#[op2(async)]
#[string]
pub async fn op_next_state_change(
    state: Rc<RefCell<OpState>>,
) -> Result<Option<String>, JsErrorBox> {
    let (rx_arc, instance_id) = {
        let s = state.borrow();
        (
            s.borrow::<EventRx>().clone(),
            s.borrow::<InstanceId>().clone(),
        )
    };
    loop {
        let event = {
            let mut guard = rx_arc.lock().await;
            match guard.recv().await {
                Ok(e) => e,
                Err(_) => return Ok(None),
            }
        };
        if let Event::StatusChanged { instance_id: eid, status } = event {
            if eid == instance_id {
                return Ok(Some(status.to_string()));
            }
        }
    }
}

deno_core::extension!(
    amberite_events,
    ops = [op_next_instance_output, op_next_state_change],
    docs = "Amberite event subscription ops"
);
