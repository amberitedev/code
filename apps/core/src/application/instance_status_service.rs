use std::{path::PathBuf, sync::Arc};

use tokio::time::{sleep, Duration};
use tracing::warn;

use crate::{
    application::{instance_service::InstanceError, installation_service::installation_dir, state::AppState},
    domain::instance::{InstanceId, InstanceInstallStatus, InstanceRecord, InstanceStatus, MemorySettings},
    domain::server_installation::InstallationId,
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

    // Integrity check: a `Ready` status only means the install task succeeded
    // once — it does not guarantee the shared files are still on disk. Verify the
    // launchable files exist before spawning, so a deleted/corrupt shared
    // installation surfaces a clear "repair" error instead of a cryptic JVM
    // failure. Legacy instances (no installation) keep their files in the data
    // dir and are validated the same way.
    let base_dir = launch_base_dir(state, &record);
    if !launch_files_present(&base_dir).await {
        return Err(InstanceError::NotReady(
            "server files are missing — repair this instance".to_string(),
        ));
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

/// Assembles the full JVM argument vector: memory flags, custom `jvm_extra`,
/// the pre-resolved launch-target tokens, then custom `server_extra`.
fn assemble_args(
    memory: &MemorySettings,
    jvm_extra: &[String],
    target: &[String],
    server_extra: &[String],
) -> Vec<String> {
    let mut args = vec![
        format!("-Xms{}m", memory.min_mb),
        format!("-Xmx{}m", memory.max_mb),
    ];
    args.extend(jvm_extra.iter().cloned());
    args.extend(target.iter().cloned());
    args.extend(server_extra.iter().cloned());
    args
}

/// Directory holding the launchable server files for a record: the shared
/// installation dir when bound, otherwise the instance's own data dir (legacy).
fn launch_base_dir(state: &Arc<AppState>, record: &InstanceRecord) -> PathBuf {
    match &record.installation_id {
        Some(id) => installation_dir(state, &InstallationId(id.clone())),
        None => PathBuf::from(&record.data_dir),
    }
}

/// Verify that the launchable server files actually exist under `base_dir`.
///
/// Uses the persisted `launch.json` to know exactly which jar/args file the
/// server needs, then confirms that file is present. Falls back to checking for
/// a `server.jar` when no `launch.json` exists (legacy instances). This is the
/// "check" step that protects `start_instance` from a deleted or half-installed
/// shared installation.
async fn launch_files_present(base_dir: &Path) -> bool {
    match read_launch_config(base_dir).await.map(|c| c.style) {
        Some(LaunchStyle::Jar { jar }) => base_dir.join(jar).is_file(),
        Some(LaunchStyle::ArgsFile { args }) => base_dir.join(args).is_file(),
        Some(LaunchStyle::Modular { args }) => modular_files_present(&args),
        None => base_dir.join("server.jar").is_file(),
    }
}

/// Platform classpath separator used to split a `Modular` classpath token back
/// into individual entries. On Windows it is `;` (jar paths never contain it);
/// elsewhere `:` (POSIX paths never contain it).
fn classpath_separator() -> char {
    if cfg!(windows) {
        ';'
    } else {
        ':'
    }
}

/// Best-effort integrity check for a `Modular` launch: every absolute `.jar`
/// path referenced in the tokens (including each classpath entry) must exist in
/// the shared store. Reaching the end means all referenced jars are present;
/// the persisted `launch.json` is only written after a successful install, so
/// no jar references at all is also treated as present.
fn modular_files_present(args: &[String]) -> bool {
    let sep = classpath_separator();
    for token in args {
        for part in token.split(sep) {
            let path = Path::new(part);
            if part.ends_with(".jar")
                && path.is_absolute()
                && !path.is_file()
            {
                return false;
            }
        }
    }
    true
}

/// Resolves the jar/args-file launch tokens (the portion between the JVM flags
/// and the server args) for a record.
///
/// For shared installations the server jar lives in the installation dir, so its
/// path is made absolute and the process working dir stays the instance data dir
/// (worlds/logs/properties land per-instance). Forge/NeoForge args files store
/// relative `libraries/` refs that only resolve from the install dir, so they are
/// rewritten to absolute paths into a per-instance `launch_args.txt`.
async fn resolve_target_tokens(
    record: &InstanceRecord,
    base_dir: &Path,
    data_dir: &Path,
) -> Vec<String> {
    let style = read_launch_config(base_dir).await.map(|c| c.style);
    match style {
        Some(LaunchStyle::Jar { jar }) => vec![
            "-jar".to_string(),
            base_dir.join(&jar).display().to_string(),
            "--nogui".to_string(),
        ],
        Some(LaunchStyle::Modular { args }) => args,
        Some(LaunchStyle::ArgsFile { args }) => {
            if record.installation_id.is_some() {
                match rewrite_args_file(base_dir, &args, data_dir).await {
                    Some(path) => vec![format!("@{}", path.display())],
                    None => vec![format!(
                        "@{}",
                        base_dir.join(&args).display()
                    )],
                }
            } else {
                // Legacy: relative args file resolves from the data dir (cwd).
                vec![format!("@{args}")]
            }
        }
        None => vec![
            "-jar".to_string(),
            data_dir.join("server.jar").display().to_string(),
            "--nogui".to_string(),
        ],
    }
}

/// Reads a shared installation's Forge/NeoForge args file, rewrites every
/// relative `libraries/` reference to an absolute path under the installation
/// dir, and writes the result to `{data_dir}/launch_args.txt`. Returns that path,
/// or `None` if the source args file could not be read.
async fn rewrite_args_file(
    base_dir: &Path,
    rel_args: &str,
    data_dir: &Path,
) -> Option<PathBuf> {
    let src = base_dir.join(rel_args);
    let content = tokio::fs::read_to_string(&src).await.ok()?;
    let abs = base_dir.display().to_string().replace('\\', "/");
    let prefix = format!("{abs}/libraries/");
    let rewritten = content
        .replace("libraries/", &prefix)
        .replace("libraries\\", &prefix);
    let dest = data_dir.join("launch_args.txt");
    tokio::fs::write(&dest, rewritten).await.ok()?;
    Some(dest)
}

/// Resolves the Java binary and full launch arguments (with the record's custom
/// overrides applied) for an instance. Shared by `start_instance` and the
/// startup-settings endpoint so the effective command never drifts.
pub(crate) async fn resolve_launch(
    state: &Arc<AppState>,
    record: &InstanceRecord,
) -> (PathBuf, Vec<String>) {
    let req_java = required_java_version(&record.game_version);
    let java = find_java_path(state, req_java)
        .await
        .unwrap_or_else(|| PathBuf::from("java"));
    let data_dir = PathBuf::from(&record.data_dir);
    let base_dir = launch_base_dir(state, record);
    let target = resolve_target_tokens(record, &base_dir, &data_dir).await;
    let jvm_extra = record.jvm_args.as_deref().map(split_args).unwrap_or_default();
    let server_extra = record
        .server_args
        .as_deref()
        .map(split_args)
        .unwrap_or_default();
    let args = assemble_args(&record.memory, &jvm_extra, &target, &server_extra);
    (java, args)
}

/// Builds the override-free default invocation arguments for display, so the UI
/// can show users what Core runs by default and offer a reset baseline.
pub(crate) async fn default_launch_args(
    state: &Arc<AppState>,
    record: &InstanceRecord,
) -> (PathBuf, Vec<String>) {
    let req_java = required_java_version(&record.game_version);
    let java = find_java_path(state, req_java)
        .await
        .unwrap_or_else(|| PathBuf::from("java"));
    let data_dir = PathBuf::from(&record.data_dir);
    let base_dir = launch_base_dir(state, record);
    let target = resolve_target_tokens(record, &base_dir, &data_dir).await;
    let args = assemble_args(&record.memory, &[], &target, &[]);
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
