use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::{timeout, Duration, Instant};
use tracing::{error, info, warn};

use crate::{
    application::state::AppState,
    domain::{
        event::Event,
        instance::{InstanceId, InstanceStatus},
    },
    ports::process_spawner::ProcessHandle,
};

/// Commands sent to the per-instance actor task.
pub enum ActorCmd {
    SendCommand(String),
    GracefulStop,
    Kill,
}

/// Handle stored in `AppState.instances` for a running instance.
pub struct InstanceHandle {
    pub cmd_tx: mpsc::Sender<ActorCmd>,
    pub pid: Option<u32>,
    pub started_at: Instant,
}

/// Spawn an actor task for a running instance.
/// Returns an `InstanceHandle` to communicate with it.
pub fn spawn_actor<H: ProcessHandle>(
    instance_id: InstanceId,
    handle: H,
    state: Arc<AppState>,
) -> InstanceHandle {
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    let pid = handle.pid();
    tokio::spawn(run_actor(instance_id, handle, cmd_rx, state));
    InstanceHandle { cmd_tx, pid, started_at: Instant::now() }
}

async fn run_actor<H: ProcessHandle>(
    instance_id: InstanceId,
    mut handle: H,
    mut cmd_rx: mpsc::Receiver<ActorCmd>,
    state: Arc<AppState>,
) {
    let Some(mut stdout_rx) = handle.take_stdout_rx() else {
        error!("No stdout rx for instance {instance_id}");
        return;
    };

    info!("Actor started for instance {instance_id}");

    loop {
        tokio::select! {
            Some(line) = stdout_rx.recv() => {
                // Minecraft signals readiness with "Done ("
                if line.contains("Done (") && matches!(
                    state.instances.get(&instance_id).map(|_| true),
                    Some(true)
                ) {
                    set_status(&state, &instance_id, InstanceStatus::Running).await;
                }
                state.broadcaster.send(Event::InstanceOutput {
                    instance_id: instance_id.clone(),
                    line,
                });
            }
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    ActorCmd::SendCommand(c) => {
                        if let Err(e) = handle.send_stdin(&c) {
                            warn!("send_stdin failed for {instance_id}: {e}");
                        }
                    }
                    ActorCmd::GracefulStop => {
                        set_status(&state, &instance_id, InstanceStatus::Stopping).await;
                        let _ = handle.send_stdin("stop");
                        let did_exit = timeout(
                            Duration::from_secs(30),
                            drain_until_closed(&mut stdout_rx),
                        ).await;
                        if did_exit.is_err() {
                            warn!("Graceful stop timed out for {instance_id}, killing");
                            let _ = handle.kill();
                        }
                        break;
                    }
                    ActorCmd::Kill => {
                        let _ = handle.kill();
                        break;
                    }
                }
            }
            else => break,
        }

        if !handle.is_running() {
            break;
        }
    }

    // Determine final status.
    let final_status = if handle.is_running() {
        let _ = handle.kill();
        InstanceStatus::Crashed
    } else {
        InstanceStatus::Offline
    };

    set_status(&state, &instance_id, final_status).await;
    state.instances.remove(&instance_id);
    info!("Actor exited for instance {instance_id}");
}

async fn set_status(state: &Arc<AppState>, id: &InstanceId, status: InstanceStatus) {
    let _ = sqlx::query("UPDATE instances SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status.to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&state.pool)
        .await;
    state.broadcaster.send(Event::StatusChanged {
        instance_id: id.clone(),
        status,
    });
}

/// Drain stdout until the channel closes (process exited).
async fn drain_until_closed(rx: &mut mpsc::Receiver<String>) {
    while rx.recv().await.is_some() {}
}
