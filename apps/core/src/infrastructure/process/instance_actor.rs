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
/// Register the handle before the actor can observe a fast startup failure.
pub fn spawn_actor<H: ProcessHandle>(
    instance_id: InstanceId,
    handle: H,
    state: Arc<AppState>,
) {
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    let pid = handle.pid();
    state.instances.insert(
        instance_id.clone(),
        InstanceHandle {
            cmd_tx,
            pid,
            started_at: Instant::now(),
        },
    );
    tokio::spawn(run_actor(instance_id, handle, cmd_rx, state));
}

async fn run_actor<H: ProcessHandle>(
    instance_id: InstanceId,
    mut handle: H,
    mut cmd_rx: mpsc::Receiver<ActorCmd>,
    state: Arc<AppState>,
) {
    let mut stdout_rx = handle.take_stdout_rx().unwrap_or_else(|| {
        error!("No stdout rx for instance {instance_id}");
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        rx
    });

    info!("Actor started for instance {instance_id}");
    let started_at = Instant::now();
    let mut expected_shutdown = false;
    let mut ready = false;
    let mut stop_deadline = None;
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            line = stdout_rx.recv() => {
                let Some(line) = line else { break; };
                // Minecraft signals readiness with "Done ("
                if line.contains("Done (") && !ready && !expected_shutdown {
                    ready = true;
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
                        expected_shutdown = true;
                        set_status(&state, &instance_id, InstanceStatus::Stopping).await;
                        // A starting JVM cannot be trusted to consume Minecraft commands.
                        if !ready || handle.send_stdin("stop").is_err() {
                            let _ = handle.kill();
                            break;
                        }
                        if stop_deadline.is_none() {
                            stop_deadline = Some(Instant::now() + Duration::from_secs(30));
                        }
                    }
                    ActorCmd::Kill => {
                        expected_shutdown = true;
                        set_status(&state, &instance_id, InstanceStatus::Stopping).await;
                        let _ = handle.kill();
                        break;
                    }
                }
            }
            _ = tick.tick() => {
                if !handle.is_running() { break; }
                if stop_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    warn!("Graceful stop timed out for {instance_id}, killing");
                    let _ = handle.kill();
                    break;
                }
            }
            else => break,
        }
    }

    if handle.is_running() {
        let _ = handle.kill();
    }
    // Forward the last stderr lines even if the exit poll won the select.
    let _ = timeout(Duration::from_secs(1), async {
        while let Some(line) = stdout_rx.recv().await {
            state.broadcaster.send(Event::InstanceOutput {
                instance_id: instance_id.clone(),
                line,
            });
        }
    })
    .await;
    let exit = handle
        .exit_status()
        .map(|status| status.to_string())
        .unwrap_or_else(|| "unknown exit status".to_string());
    let context = if expected_shutdown {
        "stopped"
    } else if ready {
        "exited"
    } else {
        "exited during startup"
    };
    state.broadcaster.send(Event::InstanceOutput {
        instance_id: instance_id.clone(),
        line: format!("[Core] Server {context} ({exit})"),
    });

    let session_secs = started_at.elapsed().as_secs();
    if session_secs > 0 {
        if let Err(e) = state
            .instance_store
            .add_uptime(&instance_id, session_secs)
            .await
        {
            warn!("Failed to persist uptime for {instance_id}: {e}");
        }
    }

    let operation_lock = state
        .instance_operation_locks
        .entry(instance_id.clone())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let guard = operation_lock.lock().await;
    state.instances.remove(&instance_id);
    set_status(&state, &instance_id, InstanceStatus::Offline).await;
    drop(guard);
    crate::application::server_source_service::on_stopped(&state, &instance_id);
    info!("Actor exited for instance {instance_id}: {context} ({exit})");
}

async fn set_status(
    state: &Arc<AppState>,
    id: &InstanceId,
    status: InstanceStatus,
) {
    if let Err(error) =
        state.instance_store.update_status(id, status.clone()).await
    {
        error!("Failed to persist {status} for instance {id}: {error}");
    }
    state.broadcaster.send(Event::StatusChanged {
        instance_id: id.clone(),
        status,
    });
}
