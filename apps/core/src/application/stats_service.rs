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
    pub ram_total_mb: Option<u64>,
    pub player_count: Option<u32>,
    pub uptime_seconds: Option<u64>,
    /// Accumulated total uptime across all sessions, in seconds.
    pub total_uptime_seconds: Option<u64>,
    /// Total on-disk size of the instance's data directory, in bytes.
    /// Computed for both running and offline instances.
    pub storage_bytes: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("instance not found")]
    NotFound,
}

pub async fn get_stats(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<StatsResponse, StatsError> {
    let uid = instance_id
        .parse::<uuid::Uuid>()
        .map_err(|_| StatsError::NotFound)?;
    let iid = InstanceId(uid);

    // Storage is computed for both running and offline instances, so always
    // resolve the record's data_dir first (also doubles as the 404 check).
    let record = state
        .instance_store
        .get(&iid)
        .await
        .map_err(|_| StatsError::NotFound)?;
    let storage_bytes = compute_dir_size(&record.data_dir).await;

    let handle = match state.instances.get(&iid) {
        Some(h) => h,
        None => {
            // BEH-05: distinguish "not in DB" (→ 404) from "offline" (→ 200 with nulls)
            return Ok(StatsResponse {
                cpu_percent: None,
                memory_mb: None,
                ram_total_mb: None,
                player_count: None,
                uptime_seconds: None,
                total_uptime_seconds: Some(record.total_uptime_seconds),
                storage_bytes,
            });
        }
    };

    let uptime_seconds = Some(handle.started_at.elapsed().as_secs());
    let total_uptime_seconds =
        Some(record.total_uptime_seconds + uptime_seconds.unwrap_or(0));
    let pid = handle.pid;
    let cmd_tx = handle.cmd_tx.clone();
    drop(handle); // release DashMap guard before any await

    let (cpu_percent, memory_mb, ram_total_mb) = if let Some(pid_val) = pid {
        tokio::task::spawn_blocking(move || {
            let p = Pid::from(pid_val as usize);
            let mut sys = System::new_all();
            std::thread::sleep(Duration::from_millis(200));
            sys.refresh_all();
            if let Some(proc) = sys.process(p) {
                let core_count = sys.cpus().len().max(1) as f32;
                (
                    Some(proc.cpu_usage() / core_count),
                    Some(proc.memory() / 1_048_576),
                    Some(sys.total_memory() / 1_048_576),
                )
            } else {
                (None, None, None)
            }
        })
        .await
        .unwrap_or((None, None, None))
    } else {
        (None, None, None)
    };

    let player_count = get_player_count(state, &iid, cmd_tx).await;

    Ok(StatsResponse {
        cpu_percent,
        memory_mb,
        ram_total_mb,
        player_count,
        uptime_seconds,
        total_uptime_seconds,
        storage_bytes,
    })
}

/// Recursively sum the byte size of all files under `dir`. Returns `None` on error
/// (e.g. the directory does not exist yet). Runs on a blocking thread.
async fn compute_dir_size(dir: &str) -> Option<u64> {
    let dir = dir.to_string();
    tokio::task::spawn_blocking(move || {
        fn walk(path: &std::path::Path) -> u64 {
            let mut total = 0;
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let Ok(meta) = entry.metadata() else { continue };
                    if meta.is_dir() {
                        total += walk(&entry.path());
                    } else {
                        total += meta.len();
                    }
                }
            }
            total
        }
        let p = std::path::Path::new(&dir);
        if p.exists() {
            Some(walk(p))
        } else {
            None
        }
    })
    .await
    .ok()
    .flatten()
}

async fn get_player_count(
    state: &Arc<AppState>,
    instance_id: &InstanceId,
    cmd_tx: tokio::sync::mpsc::Sender<ActorCmd>,
) -> Option<u32> {
    let mut rx = state.broadcaster.subscribe();
    let _ = cmd_tx.send(ActorCmd::SendCommand("list".into())).await;
    let iid = instance_id.clone();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let remaining =
            deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(Event::InstanceOutput {
                instance_id: evid,
                line,
            })) if evid == iid => {
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

#[cfg(test)]
mod tests {
    use super::parse_player_count;

    #[test]
    fn parse_normal_count() {
        assert_eq!(
            parse_player_count(
                "There are 3 of a max of 20 players online: foo, bar, baz"
            ),
            Some(3)
        );
    }

    #[test]
    fn parse_zero_players() {
        assert_eq!(
            parse_player_count("There are 0 of a max of 20 players online:"),
            Some(0)
        );
    }

    #[test]
    fn parse_malformed_line() {
        // "There are" present but no parseable number in position 2
        assert_eq!(parse_player_count("There are players online"), None);
    }

    #[test]
    fn parse_empty_string() {
        assert_eq!(parse_player_count(""), None);
    }

    #[test]
    fn parse_unrelated_line() {
        assert_eq!(
            parse_player_count("[Server] Done (2.5s)! For help, type help"),
            None
        );
    }
}
