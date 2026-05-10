//! Core instance CRUD and lifecycle management

use crate::error::{AmberiteError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

fn require_url(core_url: &Option<String>) -> Result<&str> {
    core_url
        .as_deref()
        .ok_or(AmberiteError::CoreNotConnected)
}

/// Brief instance summary
#[derive(Debug, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub game_version: String,
    pub loader: String,
    pub status: String,
}

/// Full instance details
#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceDetail {
    pub id: String,
    pub name: String,
    pub game_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub status: String,
    pub data_dir: String,
}

/// Request body for creating an instance — must match Core's CreateBody
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInstanceRequest {
    pub name: String,
    pub game_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub port: u16,
    pub memory: Option<MemorySettings>,
}

/// Memory allocation settings for a Core instance
#[derive(Debug, Serialize, Deserialize)]
pub struct MemorySettings {
    pub minimum: Option<u32>,
    pub maximum: u32,
}

pub async fn get_instances(client: &Client, core_url: &Option<String>) -> Result<Vec<Instance>> {
    let url = format!("{}/instances", require_url(core_url)?);
    Ok(client.get(&url).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

pub async fn get_instance(
    client: &Client,
    core_url: &Option<String>,
    id: &str,
) -> Result<InstanceDetail> {
    let url = format!("{}/instances/{}", require_url(core_url)?, id);
    Ok(client.get(&url).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

pub async fn create_instance(
    client: &Client,
    core_url: &Option<String>,
    req: CreateInstanceRequest,
) -> Result<InstanceDetail> {
    let url = format!("{}/instances", require_url(core_url)?);
    Ok(client.post(&url).json(&req).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

pub async fn delete_instance(
    client: &Client,
    core_url: &Option<String>,
    id: &str,
) -> Result<()> {
    let url = format!("{}/instances/{}", require_url(core_url)?, id);
    client.delete(&url).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(())
}

async fn instance_action(
    client: &Client,
    core_url: &Option<String>,
    id: &str,
    action: &str,
) -> Result<()> {
    let url = format!("{}/instances/{}/{}", require_url(core_url)?, id, action);
    client.post(&url).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(())
}

pub async fn start_instance(client: &Client, core_url: &Option<String>, id: &str) -> Result<()> {
    instance_action(client, core_url, id, "start").await
}

pub async fn stop_instance(client: &Client, core_url: &Option<String>, id: &str) -> Result<()> {
    instance_action(client, core_url, id, "stop").await
}

pub async fn kill_instance(client: &Client, core_url: &Option<String>, id: &str) -> Result<()> {
    instance_action(client, core_url, id, "kill").await
}

pub async fn restart_instance(
    client: &Client,
    core_url: &Option<String>,
    id: &str,
) -> Result<()> {
    instance_action(client, core_url, id, "restart").await
}

pub async fn send_command(
    client: &Client,
    core_url: &Option<String>,
    id: &str,
    command: &str,
) -> Result<()> {
    let url = format!("{}/instances/{}/command", require_url(core_url)?, id);
    let body = serde_json::json!({ "command": command });
    client.post(&url).json(&body).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(())
}
