//! Shared server installation orchestration.
//!
//! Key functions: installation directory resolution (line 34),
//! ensure/repair entrypoints (lines 48 and 119), install task spawning and
//! validation (lines 170 and 214), startup restore/reconcile (lines 260 and
//! 281), and status propagation helpers (line 356 onward).

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::Utc;
use tracing::{error, info};

use crate::{
    application::{instance_service::InstanceError, state::AppState},
    domain::{
        event::Event,
        instance::{InstanceId, InstanceInstallStatus, ModLoader},
        server_installation::{
            installation_key, InstallationId, InstallationStatus,
            ServerInstallationRecord,
        },
    },
    infrastructure::minecraft::{
        installer::{read_launch_config, LaunchStyle},
        java::required_java_version, server_jar::download_server_jar,
    },
};

/// Absolute path to the shared files for an installation:
/// `{data_dir}/installations/{id}/`.
pub fn installation_dir(state: &AppState, id: &InstallationId) -> PathBuf {
    state
        .config
        .data_dir
        .join("installations")
        .join(id.to_string())
}

/// Ensure a shared installation exists for the given build triple.
///
/// Returns the installation id and its current status. Spawns a background
/// install when the installation is brand new or retrying after a failure. An
/// already-`Ready` installation returns immediately with no work — this is what
/// lets a second instance of the same build come up instantly.
pub async fn ensure_installation(
    state: &Arc<AppState>,
    game_version: &str,
    loader: &ModLoader,
    loader_version: Option<&str>,
) -> Result<(InstallationId, InstallationStatus), InstanceError> {
    let id =
        InstallationId(installation_key(loader, game_version, loader_version));

    if let Some(existing) = state.installation_store.get(&id).await? {
        match existing.status {
            InstallationStatus::Ready => {
                return Ok((id, InstallationStatus::Ready));
            }
            InstallationStatus::Installing => {
                return Ok((id, InstallationStatus::Installing));
            }
            InstallationStatus::Failed => {
                // Retry a previously failed install.
                start_install(
                    state,
                    &id,
                    game_version,
                    loader,
                    loader_version,
                    "Reinstalling server",
                )
                .await?;
                return Ok((id, InstallationStatus::Installing));
            }
        }
    }

    let now = Utc::now();
    let record = ServerInstallationRecord {
        id: id.clone(),
        game_version: game_version.to_string(),
        loader: loader.clone(),
        loader_version: loader_version.map(String::from),
        status: InstallationStatus::Installing,
        error: None,
        created_at: now,
        updated_at: now,
    };
    // On a concurrent create the PK insert loses the race — fall back to the
    // existing row rather than erroring out the instance creation.
    if state.installation_store.create(&record).await.is_err() {
        return Ok((id, InstallationStatus::Installing));
    }

    broadcast_installation(
        state,
        &id,
        InstallationStatus::Installing,
        Some("Installing server".to_string()),
    );
    spawn_installation_install(
        Arc::clone(state),
        id.clone(),
        game_version.to_string(),
        loader.clone(),
        loader_version.map(String::from),
    );
    Ok((id, InstallationStatus::Installing))
}

/// Force a (re)install of an existing installation's shared files ("repair").
///
/// Marks the installation and every instance bound to it as `Installing`, then
/// respawns the install. Because the files are shared, repairing fixes all
/// instances that reference this build at once.
pub async fn repair_installation(
    state: &Arc<AppState>,
    id: &InstallationId,
) -> Result<(), InstanceError> {
    let existing =
        state.installation_store.get(id).await?.ok_or_else(|| {
            InstanceError::NotReady(format!("installation {id} not found"))
        })?;
    mark_instances_installing(state, id).await;
    start_install(
        state,
        id,
        &existing.game_version,
        &existing.loader,
        existing.loader_version.as_deref(),
        "Repairing server installation",
    )
    .await
}

/// Set an installation to `Installing`, broadcast it, and spawn the install task.
async fn start_install(
    state: &Arc<AppState>,
    id: &InstallationId,
    game_version: &str,
    loader: &ModLoader,
    loader_version: Option<&str>,
    message: &str,
) -> Result<(), InstanceError> {
    state
        .installation_store
        .update_status(id, InstallationStatus::Installing, None)
        .await?;
    broadcast_installation(
        state,
        id,
        InstallationStatus::Installing,
        Some(message.to_string()),
    );
    spawn_installation_install(
        Arc::clone(state),
        id.clone(),
        game_version.to_string(),
        loader.clone(),
        loader_version.map(String::from),
    );
    Ok(())
}

/// Background task: download/install the shared server files into the
/// installation directory, then propagate the result to every bound instance.
pub fn spawn_installation_install(
    state: Arc<AppState>,
    id: InstallationId,
    game_version: String,
    loader: ModLoader,
    loader_version: Option<String>,
) {
    tokio::spawn(async move {
        let dir = installation_dir(&state, &id);
        // Start from a clean directory so a repair/retry never reinstalls over
        // stale files from a previous (possibly half-finished) install. The
        // installer-based loaders in particular do not tolerate a dirty target.
        let _ = tokio::fs::remove_dir_all(&dir).await;
        if let Err(e) = tokio::fs::create_dir_all(&dir).await {
            finish_failed(&state, &id, &e.to_string()).await;
            return;
        }

        let req_java = required_java_version(&game_version);
        let java_path = state.java_store.find_by_version(req_java).await;
        let result = download_server_jar(
            &state.http,
            &loader,
            &game_version,
            loader_version.as_deref(),
            &dir,
            &state.config.data_dir,
            java_path.as_deref(),
        )
        .await;

        match result {
            Ok(_) => {
                if let Err(e) = validate_installation_output(&dir).await {
                    finish_failed(&state, &id, &e).await;
                } else {
                    finish_ready(&state, &id).await;
                }
            }
            Err(e) => finish_failed(&state, &id, &e.to_string()).await,
        }
    });
}

async fn validate_installation_output(dir: &Path) -> Result<(), String> {
    let config = read_launch_config(dir)
        .await
        .ok_or_else(|| "installation did not produce launch.json".to_string())?;
    match config.style {
        LaunchStyle::Jar { jar } => {
            let path = dir.join(&jar);
            if path.is_file() {
                Ok(())
            } else {
                Err(format!("launch jar is missing: {}", path.display()))
            }
        }
        LaunchStyle::ArgsFile { args } => {
            let path = dir.join(&args);
            if path.is_file() {
                Ok(())
            } else {
                Err(format!("launch args file is missing: {}", path.display()))
            }
        }
        LaunchStyle::Modular { args } => {
            if modular_launch_files_present(&args) {
                Ok(())
            } else {
                Err("shared launch files are missing".to_string())
            }
        }
    }
}

fn modular_launch_files_present(args: &[String]) -> bool {
    let separator = if cfg!(windows) { ';' } else { ':' };
    for token in args {
        for part in token.split(separator) {
            let path = Path::new(part);
            if part.ends_with(".jar") && path.is_absolute() && !path.is_file() {
                return false;
            }
        }
    }
    true
}

/// On startup, respawn installs for any installation left stuck in `Installing`
/// by an unclean shutdown.
pub async fn restore_installations(state: Arc<AppState>) {
    let stuck = state
        .installation_store
        .list_by_status(InstallationStatus::Installing)
        .await
        .unwrap_or_default();
    for inst in stuck {
        info!("Resuming interrupted installation {}", inst.id);
        spawn_installation_install(
            Arc::clone(&state),
            inst.id,
            inst.game_version,
            inst.loader,
            inst.loader_version,
        );
    }
}

/// Set an instance's `install_status` to match its installation's current
/// status. Called right after binding so the instance reflects an
/// already-`Ready` installation immediately and closes the create/install race.
pub async fn reconcile_instance(
    state: &Arc<AppState>,
    instance_id: &InstanceId,
    installation_id: &InstallationId,
) {
    let Ok(Some(inst)) = state.installation_store.get(installation_id).await
    else {
        return;
    };
    let target = match inst.status {
        InstallationStatus::Ready => InstanceInstallStatus::Ready,
        InstallationStatus::Failed => InstanceInstallStatus::Failed,
        InstallationStatus::Installing => InstanceInstallStatus::Installing,
    };
    let _ = state
        .instance_store
        .update_install_status(instance_id, target.clone())
        .await;
    state.broadcaster.send(Event::InstallStatusChanged {
        instance_id: instance_id.clone(),
        install_status: target,
        message: inst.error.clone(),
    });
}

async fn finish_ready(state: &Arc<AppState>, id: &InstallationId) {
    let _ = state
        .installation_store
        .update_status(id, InstallationStatus::Ready, None)
        .await;
    broadcast_installation(
        state,
        id,
        InstallationStatus::Ready,
        Some("Installation ready".to_string()),
    );
    info!("Installation {id} ready");
    propagate_to_instances(
        state,
        id,
        InstanceInstallStatus::Ready,
        Some("Server ready".to_string()),
        true,
    )
    .await;
}

async fn finish_failed(
    state: &Arc<AppState>,
    id: &InstallationId,
    error: &str,
) {
    let _ = state
        .installation_store
        .update_status(id, InstallationStatus::Failed, Some(error))
        .await;
    broadcast_installation(
        state,
        id,
        InstallationStatus::Failed,
        Some(error.to_string()),
    );
    error!("Installation {id} failed: {error}");
    propagate_to_instances(
        state,
        id,
        InstanceInstallStatus::Failed,
        Some(error.to_string()),
        false,
    )
    .await;
}

/// Update every instance bound to this installation that is still `Installing`,
/// pushing the terminal status and an optional completion-progress frame.
async fn propagate_to_instances(
    state: &Arc<AppState>,
    id: &InstallationId,
    status: InstanceInstallStatus,
    message: Option<String>,
    send_progress: bool,
) {
    let instances = state
        .instance_store
        .list_by_installation(&id.to_string())
        .await
        .unwrap_or_default();
    for inst in instances {
        if inst.install_status != InstanceInstallStatus::Installing {
            continue;
        }
        let _ = state
            .instance_store
            .update_install_status(&inst.id, status.clone())
            .await;
        if send_progress {
            state.broadcaster.send(Event::CreationProgress {
                instance_id: inst.id.clone(),
                progress: 1.0,
                message: message.clone().unwrap_or_default(),
            });
        }
        state.broadcaster.send(Event::InstallStatusChanged {
            instance_id: inst.id.clone(),
            install_status: status.clone(),
            message: message.clone(),
        });
    }
}

/// Mark every instance bound to this installation as `Installing` (used by
/// repair, which restarts the shared install under all of them).
async fn mark_instances_installing(state: &Arc<AppState>, id: &InstallationId) {
    let instances = state
        .instance_store
        .list_by_installation(&id.to_string())
        .await
        .unwrap_or_default();
    for inst in instances {
        let _ = state
            .instance_store
            .update_install_status(&inst.id, InstanceInstallStatus::Installing)
            .await;
        state.broadcaster.send(Event::InstallStatusChanged {
            instance_id: inst.id.clone(),
            install_status: InstanceInstallStatus::Installing,
            message: Some("Repairing server installation".to_string()),
        });
    }
}

fn broadcast_installation(
    state: &Arc<AppState>,
    id: &InstallationId,
    status: InstallationStatus,
    message: Option<String>,
) {
    state.broadcaster.send(Event::InstallationStatusChanged {
        installation_id: id.to_string(),
        status,
        message,
    });
}
