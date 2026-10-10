//! RCON orchestration: resolves an instance's `server.properties`, opens an
//! authenticated [`RconClient`], and runs commands. Also provides a helper to
//! enable RCON on an instance (patching `server.properties` and generating a
//! password) which takes effect on the next server restart.
//!
//! All Minecraft servers managed by Core bind locally, so RCON connects to
//! `127.0.0.1` regardless of `server-ip`.

use std::sync::Arc;

use crate::{
    application::state::AppState,
    domain::instance::{InstanceId, InstanceStatus},
    infrastructure::minecraft::{
        rcon::{RconClient, RconError},
        server_properties::read_properties,
    },
};

const RCON_HOST: &str = "127.0.0.1";
const DEFAULT_RCON_PORT: u16 = 25575;

#[derive(Debug, thiserror::Error)]
pub enum RconServiceError {
    #[error("instance not found")]
    NotFound,
    #[error("rcon is not enabled for this instance")]
    NotEnabled,
    #[error("instance is not running")]
    NotRunning,
    #[error("rcon: {0}")]
    Rcon(#[from] RconError),
    #[error("properties: {0}")]
    Properties(
        #[from]
        crate::infrastructure::minecraft::server_properties::PropertiesError,
    ),
}

/// Resolved RCON connection parameters parsed from `server.properties`.
struct RconConfig {
    enabled: bool,
    port: u16,
    password: String,
}

async fn resolve_data_dir(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<(InstanceId, String, InstanceStatus), RconServiceError> {
    let uid = instance_id
        .parse::<uuid::Uuid>()
        .map_err(|_| RconServiceError::NotFound)?;
    let iid = InstanceId(uid);
    let record = state
        .instance_store
        .get(&iid)
        .await
        .map_err(|_| RconServiceError::NotFound)?;
    Ok((iid, record.data_dir, record.status))
}

async fn read_rcon_config(
    data_dir: &str,
) -> Result<RconConfig, RconServiceError> {
    let props = read_properties(std::path::Path::new(data_dir)).await?;
    let enabled = props
        .get("enable-rcon")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let port = props
        .get("rcon.port")
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(DEFAULT_RCON_PORT);
    let password = props.get("rcon.password").cloned().unwrap_or_default();
    Ok(RconConfig {
        enabled,
        port,
        password,
    })
}

/// Execute a single RCON command against a running instance and return the
/// server's response text.
pub async fn execute_command(
    state: &Arc<AppState>,
    instance_id: &str,
    command: &str,
) -> Result<String, RconServiceError> {
    let (iid, data_dir, status) = resolve_data_dir(state, instance_id).await?;

    if status != InstanceStatus::Running {
        return Err(RconServiceError::NotRunning);
    }
    // An actor handle must be present for the server to be reachable.
    if !state.instances.contains_key(&iid) {
        return Err(RconServiceError::NotRunning);
    }

    let cfg = read_rcon_config(&data_dir).await?;
    if !cfg.enabled || cfg.password.is_empty() {
        return Err(RconServiceError::NotEnabled);
    }

    let mut client =
        RconClient::connect(RCON_HOST, cfg.port, &cfg.password).await?;
    let response = client.exec(command).await?;
    Ok(response)
}
