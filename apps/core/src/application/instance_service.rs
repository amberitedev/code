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
    #[error("instance must be stopped before this operation")]
    MustBeOffline,
    #[error("store: {0}")]
    Store(#[from] StoreError),
    #[error("spawn: {0}")]
    Spawn(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("instance is not ready: {0}")]
    NotReady(String),
    #[error("actor channel closed — instance may have crashed")]
    ActorDead,
    #[error("invalid instance request: {0}")]
    Invalid(String),
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
    validate_create_request(state, &req).await?;
    let id = InstanceId::new();
    let data_dir = unique_instance_data_dir(state, &req.name, &id).await?;

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

async fn validate_create_request(
    state: &Arc<AppState>,
    req: &CreateInstanceRequest,
) -> Result<(), InstanceError> {
    if req.port == 0 {
        return Err(InstanceError::Invalid("port cannot be 0".into()));
    }
    if req.memory.min_mb == 0 || req.memory.max_mb == 0 {
        return Err(InstanceError::Invalid(
            "memory must be greater than 0".into(),
        ));
    }
    if req.memory.min_mb > req.memory.max_mb {
        return Err(InstanceError::Invalid(
            "memory min_mb cannot exceed max_mb".into(),
        ));
    }
    require_port_available(state, req.port, None).await?;
    Ok(())
}

async fn require_port_available(
    state: &Arc<AppState>,
    port: u16,
    except: Option<&InstanceId>,
) -> Result<(), InstanceError> {
    let count: i64 = if let Some(except) = except {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM instances WHERE port = ? AND id != ?",
        )
        .bind(port as i64)
        .bind(except.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(StoreError::Database)?
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM instances WHERE port = ?")
            .bind(port as i64)
            .fetch_one(&state.pool)
            .await
            .map_err(StoreError::Database)?
    };
    if count > 0 {
        return Err(InstanceError::Invalid(format!(
            "port {port} is already in use"
        )));
    }
    Ok(())
}

async fn unique_instance_data_dir(
    state: &Arc<AppState>,
    name: &str,
    id: &InstanceId,
) -> Result<PathBuf, InstanceError> {
    let base = state.config.data_dir.join("instances");
    tokio::fs::create_dir_all(&base).await?;
    let slug = slug_instance_name(name);
    if let Some(path) = try_create_instance_dir(&base.join(&slug)).await? {
        return Ok(path);
    }

    let id_string = id.to_string();
    let short = &id_string[..4];
    if let Some(path) =
        try_create_instance_dir(&base.join(format!("{slug}-{short}"))).await?
    {
        return Ok(path);
    }

    let mut index = 2;
    loop {
        let candidate = base.join(format!("{slug}-{short}-{index}"));
        if let Some(path) = try_create_instance_dir(&candidate).await? {
            return Ok(path);
        }
        index += 1;
    }
}

async fn try_create_instance_dir(
    candidate: &PathBuf,
) -> Result<Option<PathBuf>, InstanceError> {
    match tokio::fs::create_dir(candidate).await {
        Ok(()) => Ok(Some(candidate.clone())),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(err) => Err(InstanceError::Io(err)),
    }
}

fn slug_instance_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;
    for ch in name.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !out.is_empty() {
            out.push('-');
            last_was_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "server".to_string()
    } else {
        out
    }
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
    state
        .broadcaster
        .send(Event::InstanceUpdated { instance: updated });

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
    if port == 0 {
        return Err(InstanceError::Invalid("port cannot be 0".into()));
    }
    require_port_available(state, port, Some(id)).await?;
    state.instance_store.update_port(id, port).await?;
    Ok(())
}

/// Delete an offline instance and its local data/backup storage.
pub async fn delete_instance(
    state: &Arc<AppState>,
    id: &InstanceId,
) -> Result<(), InstanceError> {
    if state.instances.contains_key(id) {
        return Err(InstanceError::MustBeOffline);
    }
    let record = load_record(state, id).await?;
    let backup_dir =
        crate::application::backup_service::storage_dir(state, &id.to_string());
    remove_dir_if_exists(PathBuf::from(&record.data_dir)).await?;
    remove_dir_if_exists(backup_dir).await?;
    state.instance_store.delete(id).await?;
    Ok(())
}

async fn remove_dir_if_exists(path: PathBuf) -> Result<(), InstanceError> {
    match tokio::fs::remove_dir_all(&path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(InstanceError::Io(error)),
    }
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
