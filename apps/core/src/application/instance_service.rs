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

    // Download JAR in background (caller can track via SSE events).
    let state_clone = Arc::clone(state);
    let id_clone = id.clone();
    let game_version = req.game_version.clone();
    let loader = req.loader.clone();
    let loader_version = req.loader_version.clone();
    tokio::spawn(async move {
        // Find Java path for installer-based loaders (Quilt/Forge/NeoForge)
        let req_java = required_java_version(&game_version);
        let java_path = state_clone.java_store.find_by_version(req_java).await;
        let jar_result = download_server_jar(
            &state_clone.http,
            &loader,
            &game_version,
            loader_version.as_deref(),
            &data_dir,
            java_path.as_deref(),
        )
        .await;
        match jar_result {
            Ok(_) => {
                let _ = state_clone
                    .instance_store
                    .update_install_status(
                        &id_clone,
                        InstanceInstallStatus::Ready,
                    )
                    .await;
                state_clone.broadcaster.send(Event::CreationProgress {
                    instance_id: id_clone.clone(),
                    progress: 1.0,
                    message: "Server JAR downloaded".to_string(),
                });
                state_clone.broadcaster.send(Event::InstallStatusChanged {
                    instance_id: id_clone,
                    install_status: InstanceInstallStatus::Ready,
                    message: Some("Server JAR downloaded".to_string()),
                });
            }
            Err(e) => {
                let message = e.to_string();
                let _ = state_clone
                    .instance_store
                    .update_install_status(
                        &id_clone,
                        InstanceInstallStatus::Failed,
                    )
                    .await;
                state_clone.broadcaster.send(Event::InstallStatusChanged {
                    instance_id: id_clone.clone(),
                    install_status: InstanceInstallStatus::Failed,
                    message: Some(message.clone()),
                });
                error!("JAR download failed for {id_clone}: {message}");
            }
        }
    });

    Ok(id)
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
