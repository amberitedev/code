use std::{sync::Arc, time::Duration};

use serde::Serialize;
use sysinfo::{Pid, System};

use crate::{
    application::state::AppState,
    domain::{event::Event, instance::InstanceId},
    infrastructure::process::instance_actor::ActorCmd,
};

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub cpu_percent: Option<f32>,
    pub memory_mb: Option<u64>,
    pub player_count: Option<u32>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error("db: {0}")] Db(#[from] sqlx::Error),
    #[error("instance not found")] NotFound,
}

pub async fn get_stats(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<StatsResponse, StatsError> {
    let uid = instance_id
        .parse::<uuid::Uuid>()
        .map_err(|_| StatsError::NotFound)?;
    let iid = InstanceId(uid);

    let handle = match state.instances.get(&iid) {
        Some(h) => h,
        None => {
            return Ok(StatsResponse {
                cpu_percent: None,
                memory_mb: None,
                player_count: None,
                uptime_seconds: None,
            })
        }
    };

    let uptime_seconds = Some(handle.started_at.elapsed().as_secs());
    let pid = handle.pid;
    let cmd_tx = handle.cmd_tx.clone();
    drop(handle); // release DashMap guard before any await

    let (cpu_percent, memory_mb) = if let Some(pid_val) = pid {
        tokio::task::spawn_blocking(move || {
            let p = Pid::from(pid_val as usize);
            let mut sys = System::new_all();
            std::thread::sleep(Duration::from_millis(200));
            sys.refresh_all();
            if let Some(proc) = sys.process(p) {
                (Some(proc.cpu_usage()), Some(proc.memory() / 1_048_576))
            } else {
                (None, None)
            }
        })
        .await
        .unwrap_or((None, None))
    } else {
        (None, None)
    };

    let player_count = get_player_count(state, &iid, cmd_tx).await;

    Ok(StatsResponse {
        cpu_percent,
        memory_mb,
        player_count,
        uptime_seconds,
    })
}

async fn get_player_count(
    state: &Arc<AppState>,
    instance_id: &InstanceId,
    cmd_tx: tokio::sync::mpsc::Sender<ActorCmd>,
) -> Option<u32> {
    let mut rx = state.broadcaster.subscribe();
    let _ = cmd_tx
        .send(ActorCmd::SendCommand("list".into()))
        .await;
    let iid = instance_id.clone();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(Event::InstanceOutput { instance_id: evid, line })) if evid == iid => {
                if let Some(n) = parse_player_count(&line) {
                    return Some(n);
                }
            }
            _ => break,
        }
    }
    None
}

fn parse_player_count(line: &str) -> Option<u32> {
    // Minecraft: "There are 2 of a max of 20 players online: ..."
    if !line.to_lowercase().contains("there are") {
        return None;
    }
    line.split_whitespace().nth(2)?.parse().ok()
}
