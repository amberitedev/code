use std::{path::PathBuf, sync::Arc};

use tokio::time::{sleep, Duration};
use tracing::warn;

use crate::{
    application::{
        instance_service::InstanceError,
        state::AppState,
    },
    domain::instance::{InstanceId, InstanceStatus},
    infrastructure::{
        minecraft::{
            installer::{read_launch_config, LaunchStyle},
            java::required_java_version,
        },
        process::{instance_actor::spawn_actor, pty_spawner::PtySpawner},
    },
    ports::{instance_store::StoreError, process_spawner::ProcessSpawner},
};

/// Start an existing offline/crashed instance.
pub async fn start_instance(
    state: &Arc<AppState>,
    id: &InstanceId,
) -> Result<(), InstanceError> {
    if state.instances.contains_key(id) {
        return Err(InstanceError::AlreadyRunning);
    }
    // TODO(networking/playit): Start Playit.gg tunnel on instance start for public access
    // without requiring port forwarding. See https://playit.gg/api-docs
    // TODO(networking/upnp): Request UPnP port mapping on router at instance start
    // Use the igd2 crate

    let record = state.instance_store.get(id).await
        .map_err(|e| match e {
            StoreError::NotFound(_) => InstanceError::NotFound(id.clone()),
            other => InstanceError::Store(other),
        })?;

    let req_java = required_java_version(&record.game_version);
    let java = find_java_path(state, req_java).await
        .unwrap_or_else(|| PathBuf::from("java"));

    let data_dir = PathBuf::from(&record.data_dir);
    let launch_config = read_launch_config(&data_dir).await;

    let mem_min = format!("-Xms{}m", record.memory.min_mb);
    let mem_max = format!("-Xmx{}m", record.memory.max_mb);

    let args: Vec<String> = match launch_config.map(|c| c.style) {
        Some(LaunchStyle::ArgsFile { args }) => {
            // Forge 1.17+: java -server @libraries/...args.txt
            vec![mem_min, mem_max, format!("@{args}")]
        }
        Some(LaunchStyle::Jar { jar }) => {
            let jar_path = data_dir.join(&jar);
            vec![mem_min, mem_max, "-jar".to_string(), jar_path.display().to_string(), "--nogui".to_string()]
        }
        None => {
            // Fallback: look for server.jar (legacy instances without launch.json)
            let jar = data_dir.join("server.jar");
            vec![mem_min, mem_max, "-jar".to_string(), jar.display().to_string(), "--nogui".to_string()]
        }
    };

    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let handle = PtySpawner
        .spawn(java.to_str().unwrap_or("java"), &args_refs, &data_dir, &[("SERVER_PORT", &record.port.to_string())])
        .await
        .map_err(|e| InstanceError::Spawn(e.to_string()))?;

    set_status(state, id, InstanceStatus::Starting).await;
    let actor_handle = spawn_actor(id.clone(), handle, Arc::clone(state));
    state.instances.insert(id.clone(), actor_handle);
    Ok(())
}

/// Request graceful stop of a running instance.
pub async fn stop_instance(state: &Arc<AppState>, id: &InstanceId) -> Result<(), InstanceError> {
    let handle = state.instances.get(id).ok_or(InstanceError::NotRunning)?;
    let _ = handle.cmd_tx.send(crate::infrastructure::process::instance_actor::ActorCmd::GracefulStop).await;
    Ok(())
}

/// Force-kill a running instance.
pub async fn kill_instance(state: &Arc<AppState>, id: &InstanceId) -> Result<(), InstanceError> {
    let handle = state.instances.get(id).ok_or(InstanceError::NotRunning)?;
    let _ = handle.cmd_tx.send(crate::infrastructure::process::instance_actor::ActorCmd::Kill).await;
    Ok(())
}

/// Send a console command to a running instance.
pub async fn send_command(
    state: &Arc<AppState>,
    id: &InstanceId,
    cmd: String,
) -> Result<(), InstanceError> {
    let handle = state.instances.get(id).ok_or(InstanceError::NotRunning)?;
    let _ = handle.cmd_tx.try_send(crate::infrastructure::process::instance_actor::ActorCmd::SendCommand(cmd));
    Ok(())
}

/// Stop an instance and restart it — polls until stopped (30s timeout).
pub async fn restart_instance(
    state: &Arc<AppState>,
    id: &InstanceId,
) -> Result<(), InstanceError> {
    stop_instance(state, id).await?;

    // Poll until stopped or 30s timeout.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        if !state.instances.contains_key(id) {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            warn!("Restart timed out waiting for stop on {id}");
            return Err(InstanceError::Spawn("shutdown timed out".to_string()));
        }
        sleep(Duration::from_millis(500)).await;
    }

    start_instance(state, id).await
}

pub(crate) async fn set_status(state: &Arc<AppState>, id: &InstanceId, status: InstanceStatus) {
    let _ = state.instance_store.update_status(id, status.clone()).await;
    state.broadcaster.send(crate::domain::event::Event::StatusChanged {
        instance_id: id.clone(),
        status,
    });
}

async fn find_java_path(state: &Arc<AppState>, version: u32) -> Option<PathBuf> {
    crate::infrastructure::minecraft::java::find_java(&state.pool, version).await
}

