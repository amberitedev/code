//! HTTP client for Amberite Core REST API

use crate::error::{AmberiteError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Health status returned by Core `/health`
#[derive(Debug, Serialize, Deserialize)]
pub struct CoreHealth {
    pub status: String,
    pub version: String,
}

/// Metrics returned by Core `/metrics`
#[derive(Debug, Serialize, Deserialize)]
pub struct CoreMetrics {
    pub uptime_secs: u64,
    pub active_instances: u32,
    pub memory_mb: f64,
}

/// A detected Java installation
#[derive(Debug, Serialize, Deserialize)]
pub struct JavaInstallation {
    pub path: String,
    pub version: String,
}

/// Core configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct CoreConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: String,
}

fn require_url(core_url: &Option<String>) -> Result<&str> {
    core_url
        .as_deref()
        .ok_or(AmberiteError::CoreNotConnected)
}

/// GET /health
pub async fn core_health(client: &Client, core_url: &Option<String>) -> Result<CoreHealth> {
    let url = format!("{}/health", require_url(core_url)?);
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(resp.json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

/// GET /version
pub async fn core_version(client: &Client, core_url: &Option<String>) -> Result<String> {
    let url = format!("{}/version", require_url(core_url)?);
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    let h: CoreHealth = resp.json().await.map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(h.version)
}

/// GET /java
pub async fn get_java_installations(
    client: &Client,
    core_url: &Option<String>,
) -> Result<Vec<JavaInstallation>> {
    let url = format!("{}/java", require_url(core_url)?);
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(resp.json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

/// GET /metrics
pub async fn core_metrics(client: &Client, core_url: &Option<String>) -> Result<CoreMetrics> {
    let url = format!("{}/metrics", require_url(core_url)?);
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(resp.json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

/// GET /instances/:id/ws-token
pub async fn get_ws_token(
    client: &Client,
    core_url: &Option<String>,
    instance_id: &str,
) -> Result<String> {
    let url = format!("{}/instances/{}/ws-token", require_url(core_url)?, instance_id);
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    let json: serde_json::Value = resp.json().await.map_err(|e| AmberiteError::Core(e.to_string()))?;
    json["token"].as_str().map(String::from)
        .ok_or_else(|| AmberiteError::Core("No token in ws-token response".into()))
}

/// GET /config
pub async fn get_core_config(client: &Client, core_url: &Option<String>) -> Result<CoreConfig> {
    let url = format!("{}/config", require_url(core_url)?);
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(resp.json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}
