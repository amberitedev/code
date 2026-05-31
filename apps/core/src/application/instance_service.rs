use std::{path::PathBuf, sync::Arc};

use tracing::{error, info};

use crate::{
    application::state::AppState,
    domain::{
        event::Event,
        instance::{
            InstanceId, InstanceInstallStatus, InstanceRecord, InstanceStatus,
            MemorySettings, ModLoader,
        },
    },
    infrastructure::minecraft::{
        java::{detect_java_installations, required_java_version},
        server_jar::download_server_jar,
        server_properties::write_initial_properties,
    },
    ports::instance_store::StoreError,
};

pub struct CreateInstanceRequest {
    pub name: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub port: u16,
    pub memory: MemorySettings,
}

#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("not found: {0}")]
    NotFound(InstanceId),
    #[error("already running")]
    AlreadyRunning,
    #[error("not running")]
    NotRunning,
    #[error("store: {0}")]
    Store(#[from] StoreError),
    #[error("spawn: {0}")]
    Spawn(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("jar download: {0}")]
    JarDownload(String),
    #[error("instance is not ready: {0}")]
    NotReady(String),
    #[error("actor channel closed — instance may have crashed")]
    ActorDead,
}

/// Create a new instance record, write server.properties, and start JAR download.
pub async fn create_instance(
    state: &Arc<AppState>,
    req: CreateInstanceRequest,
) -> Result<InstanceId, InstanceError> {
    let id = InstanceId::new();
    let data_dir = state.config.data_dir.join("instances").join(id.to_string());
    tokio::fs::create_dir_all(&data_dir).await?;

    // B4: Write initial server.properties so the server can start on first launch.
    write_initial_properties(&data_dir, req.port)
        .await
        .map_err(|e| InstanceError::Io(std::io::Error::other(e.to_string())))?;
    // TODO(backups): Implement world backup — create a timestamped zip of {data_dir}/world/
    // See .plan/active/features.md for full backup scope and .plan/core_rewrite/README.md
    // TODO(import): accept existing server dir to populate data_dir instead of empty creation

    let now = chrono::Utc::now();
    let record = InstanceRecord {
        id: id.clone(),
        name: req.name,
        game_version: req.game_version.clone(),
        loader: req.loader.clone(),
        loader_version: req.loader_version.clone(),
        port: req.port,
        memory: req.memory,
        java_version: None,
        install_status: InstanceInstallStatus::Installing,
        status: InstanceStatus::Offline,
        data_dir: data_dir.display().to_string(),
        created_at: now,
        updated_at: now,
    };

    state.instance_store.create(&record).await?;
    state.broadcaster.send(Event::InstanceCreated {
        instance: record.clone(),
    });

    spawn_jar_install(
        Arc::clone(state),
        id.clone(),
        req.game_version.clone(),
        req.loader.clone(),
        req.loader_version.clone(),
        data_dir,
        "Server JAR downloaded",
    );

    Ok(id)
}

/// Spawn a background task that (re)downloads the server JAR for an instance and
/// updates `install_status` + broadcasts progress/status events.
///
/// Shared by `create_instance`, `repair_instance`, and `change_version`. The caller
/// is responsible for having already set `install_status` to `Installing` in the DB
/// (and broadcasting it) so the UI shows the installing state immediately.
fn spawn_jar_install(
    state: Arc<AppState>,
    id: InstanceId,
    game_version: String,
    loader: ModLoader,
    loader_version: Option<String>,
    data_dir: PathBuf,
    success_message: &'static str,
) {
    tokio::spawn(async move {
        // Find Java path for installer-based loaders (Quilt/Forge/NeoForge)
        let req_java = required_java_version(&game_version);
        let java_path = state.java_store.find_by_version(req_java).await;
        let jar_result = download_server_jar(
            &state.http,
            &loader,
            &game_version,
            loader_version.as_deref(),
            &data_dir,
            java_path.as_deref(),
        )
        .await;
        match jar_result {
            Ok(_) => {
                let _ = state
                    .instance_store
                    .update_install_status(&id, InstanceInstallStatus::Ready)
                    .await;
                state.broadcaster.send(Event::CreationProgress {
                    instance_id: id.clone(),
                    progress: 1.0,
                    message: success_message.to_string(),
                });
                state.broadcaster.send(Event::InstallStatusChanged {
                    instance_id: id.clone(),
                    install_status: InstanceInstallStatus::Ready,
                    message: Some(success_message.to_string()),
                });
                info!("JAR install complete for {id}: {success_message}");
            }
            Err(e) => {
                let message = e.to_string();
                let _ = state
                    .instance_store
                    .update_install_status(&id, InstanceInstallStatus::Failed)
                    .await;
                state.broadcaster.send(Event::InstallStatusChanged {
                    instance_id: id.clone(),
                    install_status: InstanceInstallStatus::Failed,
                    message: Some(message.clone()),
                });
                error!("JAR download failed for {id}: {message}");
            }
        }
    });
}

/// Re-download/reinstall the server JAR for an existing instance ("repair").
///
/// Refuses while the instance is running. Sets `install_status` to `Installing`,
/// broadcasts it, then spawns the shared install task. Track progress via SSE.
pub async fn repair_instance(
    state: &Arc<AppState>,
    id: &InstanceId,
) -> Result<(), InstanceError> {
    if state.instances.contains_key(id) {
        return Err(InstanceError::AlreadyRunning);
    }
    let record = state.instance_store.get(id).await.map_err(|e| match e {
        StoreError::NotFound(_) => InstanceError::NotFound(id.clone()),
        other => InstanceError::Store(other),
    })?;

    state
        .instance_store
        .update_install_status(id, InstanceInstallStatus::Installing)
        .await?;
    state.broadcaster.send(Event::InstallStatusChanged {
        instance_id: id.clone(),
        install_status: InstanceInstallStatus::Installing,
        message: Some("Repairing server installation".to_string()),
    });

    spawn_jar_install(
        Arc::clone(state),
        id.clone(),
        record.game_version,
        record.loader,
        record.loader_version,
        PathBuf::from(&record.data_dir),
        "Server repaired",
    );
    Ok(())
}

/// Change the game version and/or loader of an existing instance, then reinstall.
///
/// Refuses while the instance is running. Persists the new version fields, sets
/// `install_status` to `Installing`, broadcasts updates, then reinstalls the JAR.
pub async fn change_version(
    state: &Arc<AppState>,
    id: &InstanceId,
    game_version: Option<String>,
    loader: Option<ModLoader>,
    loader_version: Option<Option<String>>,
) -> Result<(), InstanceError> {
    if state.instances.contains_key(id) {
        return Err(InstanceError::AlreadyRunning);
    }
    let mut record = state.instance_store.get(id).await.map_err(|e| match e {
        StoreError::NotFound(_) => InstanceError::NotFound(id.clone()),
        other => InstanceError::Store(other),
    })?;

    if let Some(gv) = game_version {
        record.game_version = gv;
    }
    if let Some(l) = loader {
        record.loader = l;
    }
    if let Some(lv) = loader_version {
        record.loader_version = lv;
    }

    state
        .instance_store
        .update_version(
            id,
            &record.game_version,
            &record.loader,
            record.loader_version.as_deref(),
        )
        .await?;
    state
        .instance_store
        .update_install_status(id, InstanceInstallStatus::Installing)
        .await?;

    let updated = state.instance_store.get(id).await?;
    state.broadcaster.send(Event::InstanceUpdated {
        instance: updated.clone(),
    });
    state.broadcaster.send(Event::InstallStatusChanged {
        instance_id: id.clone(),
        install_status: InstanceInstallStatus::Installing,
        message: Some("Changing version".to_string()),
    });

    spawn_jar_install(
        Arc::clone(state),
        id.clone(),
        record.game_version,
        record.loader,
        record.loader_version,
        PathBuf::from(&record.data_dir),
        "Version changed",
    );
    Ok(())
}

/// Update the port for an instance (used when server-port is changed in properties).
pub async fn update_port(
    state: &Arc<AppState>,
    id: &InstanceId,
    port: u16,
) -> Result<(), InstanceError> {
    state.instance_store.update_port(id, port).await?;
    Ok(())
}

/// On startup, detect Java and restore any instances that were Running before shutdown.
pub async fn restore_instances(state: Arc<AppState>) {
    // Sync Java installations to DB
    let installs = detect_java_installations();
    state.java_store.sync_all(&installs).await;

    // Reset any instances stuck in transient states
    let _ = state.instance_store.reset_transient_statuses().await;

    // Restore instances that were running before Core stopped.
    let running = state
        .instance_store
        .list_by_status(InstanceStatus::Running)
        .await
        .unwrap_or_default();

    for record in running {
        info!("Restoring instance {}", record.id);
        if let Err(e) =
            crate::application::instance_status_service::start_instance(
                &state, &record.id,
            )
            .await
        {
            error!("Failed to restore instance {}: {e}", record.id);
        }
    }
}

/// Get the data directory for an instance by ID.
pub async fn get_data_dir(
    state: &Arc<AppState>,
    id: &InstanceId,
) -> Result<PathBuf, InstanceError> {
    let record = state.instance_store.get(id).await.map_err(|e| match e {
        StoreError::NotFound(_) => InstanceError::NotFound(id.clone()),
        other => InstanceError::Store(other),
    })?;
    Ok(PathBuf::from(&record.data_dir))
}
