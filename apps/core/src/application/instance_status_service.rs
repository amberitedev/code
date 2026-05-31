use std::{path::PathBuf, sync::Arc};

use tokio::time::{sleep, Duration};
use tracing::warn;

use crate::{
    application::{instance_service::InstanceError, state::AppState},
    domain::instance::{InstanceId, InstanceInstallStatus, InstanceStatus, MemorySettings},
    infrastructure::{
        minecraft::{
            installer::{read_launch_config, LaunchStyle},
            java::required_java_version,
        },
        process::instance_actor::spawn_actor,
    },
    ports::instance_store::StoreError,
};

use std::path::Path;

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

    let record = state.instance_store.get(id).await.map_err(|e| match e {
        StoreError::NotFound(_) => InstanceError::NotFound(id.clone()),
        other => InstanceError::Store(other),
    })?;

    if record.install_status != InstanceInstallStatus::Ready {
        return Err(InstanceError::NotReady(record.install_status.to_string()));
    }

    let (java, args) = resolve_launch(state, &record).await;
    let data_dir = PathBuf::from(&record.data_dir);

    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let handle = state
        .spawner
        .spawn_any(
            java.to_str().unwrap_or("java"),
            &args_refs,
            &data_dir,
            &[("SERVER_PORT", &record.port.to_string())],
        )
        .await
        .map_err(|e| InstanceError::Spawn(e.to_string()))?;

    set_status(state, id, InstanceStatus::Starting).await;
    let actor_handle = spawn_actor(id.clone(), handle, Arc::clone(state));
    state.instances.insert(id.clone(), actor_handle);
    Ok(())
}

/// Splits a user-supplied argument string into individual arguments on
/// whitespace, treating a double-quoted span as a single argument.
/// Quotes are stripped; this is a deliberately small parser (no escapes).
pub(crate) fn split_args(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in input.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Builds the full JVM argument vector for a record: memory flags, then any
/// custom `jvm_args`, then the jar/args-file launch target, then any custom
/// `server_args`. `jvm_extra`/`server_extra` are passed explicitly so callers
/// can render the override-free "default" invocation by passing empty slices.
fn build_args(
    memory: &MemorySettings,
    data_dir: &Path,
    style: Option<LaunchStyle>,
    jvm_extra: &[String],
    server_extra: &[String],
) -> Vec<String> {
    let mut args = vec![
        format!("-Xms{}m", memory.min_mb),
        format!("-Xmx{}m", memory.max_mb),
    ];
    args.extend(jvm_extra.iter().cloned());

    match style {
        Some(LaunchStyle::ArgsFile { args: argfile }) => {
            // Forge 1.17+: java -Xms -Xmx [jvm] @libraries/...args.txt
            args.push(format!("@{argfile}"));
        }
        Some(LaunchStyle::Jar { jar }) => {
            args.push("-jar".to_string());
            args.push(data_dir.join(&jar).display().to_string());
            args.push("--nogui".to_string());
        }
        None => {
            // Fallback: legacy instances without launch.json.
            args.push("-jar".to_string());
            args.push(data_dir.join("server.jar").display().to_string());
            args.push("--nogui".to_string());
        }
    }

    args.extend(server_extra.iter().cloned());
    args
}

/// Resolves the Java binary and full launch arguments (with the record's custom
/// overrides applied) for an instance. Shared by `start_instance` and the
/// startup-settings endpoint so the effective command never drifts.
pub(crate) async fn resolve_launch(
    state: &Arc<AppState>,
    record: &crate::domain::instance::InstanceRecord,
) -> (PathBuf, Vec<String>) {
    let req_java = required_java_version(&record.game_version);
    let java = find_java_path(state, req_java)
        .await
        .unwrap_or_else(|| PathBuf::from("java"));
    let data_dir = PathBuf::from(&record.data_dir);
    let style = read_launch_config(&data_dir).await.map(|c| c.style);
    let jvm_extra = record.jvm_args.as_deref().map(split_args).unwrap_or_default();
    let server_extra = record
        .server_args
        .as_deref()
        .map(split_args)
        .unwrap_or_default();
    let args = build_args(&record.memory, &data_dir, style, &jvm_extra, &server_extra);
    (java, args)
}

/// Builds the override-free default invocation arguments for display, so the UI
/// can show users what Core runs by default and offer a reset baseline.
pub(crate) async fn default_launch_args(
    state: &Arc<AppState>,
    record: &crate::domain::instance::InstanceRecord,
) -> (PathBuf, Vec<String>) {
    let req_java = required_java_version(&record.game_version);
    let java = find_java_path(state, req_java)
        .await
        .unwrap_or_else(|| PathBuf::from("java"));
    let data_dir = PathBuf::from(&record.data_dir);
    let style = read_launch_config(&data_dir).await.map(|c| c.style);
    let args = build_args(&record.memory, &data_dir, style, &[], &[]);
    (java, args)
}

/// Request graceful stop of a running instance.
pub async fn stop_instance(
    state: &Arc<AppState>,
    id: &InstanceId,
) -> Result<(), InstanceError> {
    if !state.instances.contains_key(id) {
        // BEH-06: check DB to distinguish "not found" from "offline"
        state.instance_store.get(id).await.map_err(|e| match e {
            StoreError::NotFound(_) => InstanceError::NotFound(id.clone()),
            other => InstanceError::Store(other),
        })?;
        return Err(InstanceError::NotRunning);
    }
    let handle = state.instances.get(id).ok_or(InstanceError::NotRunning)?;
    let _ = handle.cmd_tx.send(crate::infrastructure::process::instance_actor::ActorCmd::GracefulStop).await;
    Ok(())
}

/// Force-kill a running instance.
pub async fn kill_instance(
    state: &Arc<AppState>,
    id: &InstanceId,
) -> Result<(), InstanceError> {
    if !state.instances.contains_key(id) {
        // BEH-06: check DB to distinguish "not found" from "offline"
        state.instance_store.get(id).await.map_err(|e| match e {
            StoreError::NotFound(_) => InstanceError::NotFound(id.clone()),
            other => InstanceError::Store(other),
        })?;
        return Err(InstanceError::NotRunning);
    }
    let handle = state.instances.get(id).ok_or(InstanceError::NotRunning)?;
    let _ = handle
        .cmd_tx
        .send(crate::infrastructure::process::instance_actor::ActorCmd::Kill)
        .await;
    Ok(())
}

/// Send a console command to a running instance.
pub async fn send_command(
    state: &Arc<AppState>,
    id: &InstanceId,
    cmd: String,
) -> Result<(), InstanceError> {
    if !state.instances.contains_key(id) {
        // BEH-06: check DB to distinguish "not found" from "offline"
        state.instance_store.get(id).await.map_err(|e| match e {
            StoreError::NotFound(_) => InstanceError::NotFound(id.clone()),
            other => InstanceError::Store(other),
        })?;
        return Err(InstanceError::NotRunning);
    }
    let handle = state.instances.get(id).ok_or(InstanceError::NotRunning)?;
    handle.cmd_tx.send(crate::infrastructure::process::instance_actor::ActorCmd::SendCommand(cmd))
        .await
        .map_err(|_| InstanceError::ActorDead)?;
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

pub(crate) async fn set_status(
    state: &Arc<AppState>,
    id: &InstanceId,
    status: InstanceStatus,
) {
    let _ = state.instance_store.update_status(id, status.clone()).await;
    state
        .broadcaster
        .send(crate::domain::event::Event::StatusChanged {
            instance_id: id.clone(),
            status,
        });
}

async fn find_java_path(
    state: &Arc<AppState>,
    version: u32,
) -> Option<PathBuf> {
    state.java_store.find_by_version(version).await
}
