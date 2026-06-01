use std::{path::PathBuf, sync::Arc};

use tracing::{error, info};

use crate::{
    application::{
        installation_service::{
            ensure_installation, reconcile_instance, repair_installation,
            restore_installations,
        },
        state::AppState,
    },
    domain::{
        event::Event,
        instance::{
            InstanceId, InstanceInstallStatus, InstanceRecord, InstanceStatus,
            MemorySettings, ModLoader,
        },
        server_installation::InstallationId,
    },
    infrastructure::minecraft::{
        java::detect_java_installations,
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

/// Create a new instance: write its per-instance data dir + server.properties,
/// bind it to a shared installation for the requested build (installing the
/// shared server files in the background if needed), and persist the record.
///
/// Returns immediately. If the shared installation is already `Ready` the
/// instance is `Ready` at once; otherwise track progress via the SSE endpoints.
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

    let (installation_id, _status) = ensure_installation(
        state,
        &req.game_version,
        &req.loader,
        req.loader_version.as_deref(),
    )
    .await?;

    let now = chrono::Utc::now();
    let record = InstanceRecord {
        id: id.clone(),
        name: req.name,
        game_version: req.game_version,
        loader: req.loader,
        loader_version: req.loader_version,
        port: req.port,
        memory: req.memory,
        java_version: None,
        jvm_args: None,
        server_args: None,
        install_status: InstanceInstallStatus::Installing,
        status: InstanceStatus::Offline,
        data_dir: data_dir.display().to_string(),
        installation_id: Some(installation_id.to_string()),
        total_uptime_seconds: 0,
        created_at: now,
        updated_at: now,
    };

    state.instance_store.create(&record).await?;
    state.broadcaster.send(Event::InstanceCreated {
        instance: record.clone(),
    });

    // Reconcile after insert: an already-ready installation marks this instance
    // ready immediately; otherwise the running install task will propagate.
    reconcile_instance(state, &id, &installation_id).await;

    Ok(id)
}

/// Re-download/reinstall the shared server files backing an instance ("repair").
///
/// Refuses while the instance is running. Binds the instance to an installation
/// for its build (migrating legacy instances), then forces a fresh install of
/// the shared files — which repairs every instance that shares this build.
pub async fn repair_instance(
    state: &Arc<AppState>,
    id: &InstanceId,
) -> Result<(), InstanceError> {
    if state.instances.contains_key(id) {
        return Err(InstanceError::AlreadyRunning);
    }
    let record = load_record(state, id).await?;

    let (installation_id, _) = ensure_installation(
        state,
        &record.game_version,
        &record.loader,
        record.loader_version.as_deref(),
    )
    .await?;
    bind_instance(state, id, &installation_id).await?;
    repair_installation(state, &installation_id).await?;
    Ok(())
}

/// Change the game version and/or loader of an existing instance, then rebind it
/// to the appropriate shared installation (installing it if new).
///
/// Refuses while the instance is running.
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
    let mut record = load_record(state, id).await?;

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

    let (installation_id, _) = ensure_installation(
        state,
        &record.game_version,
        &record.loader,
        record.loader_version.as_deref(),
    )
    .await?;
    bind_instance(state, id, &installation_id).await?;

    let updated = state.instance_store.get(id).await?;
    state.broadcaster.send(Event::InstanceUpdated {
        instance: updated,
    });

    reconcile_instance(state, id, &installation_id).await;
    Ok(())
}

/// Bind an instance to a shared installation.
async fn bind_instance(
    state: &Arc<AppState>,
    id: &InstanceId,
    installation_id: &InstallationId,
) -> Result<(), InstanceError> {
    state
        .instance_store
        .update_installation_id(id, Some(&installation_id.to_string()))
        .await?;
    Ok(())
}

/// Fetch an instance record, mapping a missing row to `InstanceError::NotFound`.
async fn load_record(
    state: &Arc<AppState>,
    id: &InstanceId,
) -> Result<InstanceRecord, InstanceError> {
    state.instance_store.get(id).await.map_err(|e| match e {
        StoreError::NotFound(_) => InstanceError::NotFound(id.clone()),
        other => InstanceError::Store(other),
    })
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

/// On startup, detect Java, resume interrupted shared installations, and restore
/// any instances that were Running before shutdown.
pub async fn restore_instances(state: Arc<AppState>) {
    // Sync Java installations to DB
    let installs = detect_java_installations();
    state.java_store.sync_all(&installs).await;

    // Resume any shared installations interrupted by an unclean shutdown.
    restore_installations(Arc::clone(&state)).await;

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
    let record = load_record(state, id).await?;
    Ok(PathBuf::from(&record.data_dir))
}
