//! Modpack and macro management via Core REST API

use crate::error::{AmberiteError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

fn require_url(core_url: &Option<String>) -> Result<&str> {
    core_url
        .as_deref()
        .ok_or(AmberiteError::CoreNotConnected)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Modpack {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub mod_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Macro {
    pub id: String,
    pub name: String,
    pub commands: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMacroRequest {
    pub name: String,
    pub commands: Vec<String>,
}

pub async fn install_modpack(
    client: &Client,
    core_url: &Option<String>,
    instance_id: &str,
    modpack_url: &str,
) -> Result<Modpack> {
    let url = format!(
        "{}/instances/{}/modpacks",
        require_url(core_url)?,
        instance_id
    );
    let body = serde_json::json!({ "url": modpack_url });
    Ok(client.post(&url).json(&body).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

pub async fn get_modpack(
    client: &Client,
    core_url: &Option<String>,
    instance_id: &str,
    modpack_id: &str,
) -> Result<Modpack> {
    let url = format!(
        "{}/instances/{}/modpacks/{}",
        require_url(core_url)?,
        instance_id,
        modpack_id
    );
    Ok(client.get(&url).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

pub async fn remove_modpack(
    client: &Client,
    core_url: &Option<String>,
    instance_id: &str,
    modpack_id: &str,
) -> Result<()> {
    let url = format!(
        "{}/instances/{}/modpacks/{}",
        require_url(core_url)?,
        instance_id,
        modpack_id
    );
    client.delete(&url).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(())
}

pub async fn list_macros(
    client: &Client,
    core_url: &Option<String>,
    instance_id: &str,
) -> Result<Vec<Macro>> {
    let url = format!(
        "{}/instances/{}/macros",
        require_url(core_url)?,
        instance_id
    );
    Ok(client.get(&url).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

pub async fn create_macro(
    client: &Client,
    core_url: &Option<String>,
    instance_id: &str,
    req: CreateMacroRequest,
) -> Result<Macro> {
    let url = format!(
        "{}/instances/{}/macros",
        require_url(core_url)?,
        instance_id
    );
    Ok(client.post(&url).json(&req).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Core(e.to_string()))?)
}

pub async fn delete_macro(
    client: &Client,
    core_url: &Option<String>,
    instance_id: &str,
    macro_id: &str,
) -> Result<()> {
    let url = format!(
        "{}/instances/{}/macros/{}",
        require_url(core_url)?,
        instance_id,
        macro_id
    );
    client.delete(&url).send().await?.error_for_status()
        .map_err(|e| AmberiteError::Core(e.to_string()))?;
    Ok(())
}
