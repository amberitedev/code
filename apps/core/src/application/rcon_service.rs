//! RCON orchestration: resolves an instance's `server.properties`, opens an
//! authenticated [`RconClient`], and runs commands. Also provides a helper to
//! enable RCON on an instance (patching `server.properties` and generating a
//! password) which takes effect on the next server restart.
//!
//! All Minecraft servers managed by Core bind locally, so RCON connects to
//! `127.0.0.1` regardless of `server-ip`.

use std::sync::Arc;

use rand::Rng;
use serde::Serialize;

use crate::{
    application::state::AppState,
    domain::instance::{InstanceId, InstanceStatus},
    infrastructure::minecraft::{
        rcon::{RconClient, RconError},
        server_properties::{patch_properties, read_properties},
    },
};

const RCON_HOST: &str = "127.0.0.1";
const DEFAULT_RCON_PORT: u16 = 25575;
const GENERATED_PASSWORD_LEN: usize = 24;

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

/// Outcome of enabling RCON on an instance.
#[derive(Debug, Serialize)]
pub struct RconEnableResult {
    pub port: u16,
    pub password: String,
    /// True if the server is currently running and must be restarted for the
    /// new `server.properties` to take effect.
    pub restart_required: bool,
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

/// Enable RCON on an instance: ensures `enable-rcon=true`, a `rcon.port`, and a
/// generated `rcon.password` are present in `server.properties`. Returns the
/// effective port/password and whether a restart is required.
pub async fn enable_rcon(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<RconEnableResult, RconServiceError> {
    let (_iid, data_dir, status) = resolve_data_dir(state, instance_id).await?;

    let existing = read_rcon_config(&data_dir).await?;
    let password = if existing.password.is_empty() {
        generate_password()
    } else {
        existing.password
    };
    let port = existing.port;

    let mut updates = std::collections::HashMap::new();
    updates.insert("enable-rcon".to_string(), "true".to_string());
    updates.insert("rcon.port".to_string(), port.to_string());
    updates.insert("rcon.password".to_string(), password.clone());
    patch_properties(std::path::Path::new(&data_dir), &updates).await?;

    let restart_required =
        matches!(status, InstanceStatus::Running | InstanceStatus::Starting)
            || !existing.enabled;

    Ok(RconEnableResult {
        port,
        password,
        restart_required: restart_required
            && matches!(status, InstanceStatus::Running),
    })
}

fn generate_password() -> String {
    const CHARSET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..GENERATED_PASSWORD_LEN)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}
