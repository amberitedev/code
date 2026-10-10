use std::sync::Arc;

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
        instance::{InstanceId, InstanceRecord, InstanceStatus, ModLoader},
        server_installation::InstallationId,
    },
    infrastructure::minecraft::java::detect_java_installations,
    ports::instance_store::StoreError,
};

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
    #[error("instance is not ready: {0}")]
    NotReady(String),
    #[error("actor channel closed — instance may have crashed")]
    ActorDead,
    #[error("invalid instance request: {0}")]
    Invalid(String),
}

pub fn sanitize_instance_path(input: &str) -> String {
    let path = input.trim().replace(
        ['/', '\\', '?', '*', ':', '\'', '\"', '|', '<', '>', '!'],
        "_",
    );
    if path.trim().is_empty() {
        "server".to_string()
    } else {
        path
    }
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

#[cfg(test)]
mod tests {
    use super::sanitize_instance_path;

    #[test]
    fn sanitize_instance_path_replaces_profile_forbidden_characters() {
        assert_eq!(
            sanitize_instance_path("bad/\\?*:'\"|<>!name"),
            "bad___________name"
        );
    }

    #[test]
    fn sanitize_instance_path_preserves_readable_text() {
        assert_eq!(
            sanitize_instance_path("My Fabric Server"),
            "My Fabric Server"
        );
    }

    #[test]
    fn sanitize_instance_path_uses_empty_name_fallback() {
        assert_eq!(sanitize_instance_path("   "), "server");
    }
}
