//! Networking - UPnP, playit.gg tunneling, port management.

use thiserror::Error;
use reqwest::Client;

/// Networking error.
#[derive(Error, Debug)]
pub enum NetworkingError {
    #[error("UPnP not available")]
    UpnpNotAvailable,
    #[error("Port mapping failed: {0}")]
    PortMappingFailed(String),
    #[error("Playit.gg API error: {0}")]
    PlayitApiError(String),
}

/// UPnP manager.
pub struct UpnpManager {
    gateway: Option<igd::Gateway>,
}

impl UpnpManager {
    pub async fn new() -> Result<Self, NetworkingError> {
        // For now, just return a dummy instance to avoid IGD API issues
        Ok(UpnpManager { gateway: None })
    }

    pub async fn add_port_mapping(
        &self,
        _external_port: u16,
        _internal_port: u16,
        _protocol: igd::PortMappingProtocol,
        _description: &str,
    ) -> Result<(), NetworkingError> {
        // Stub implementation to avoid IGD API issues
        Ok(())
    }

    pub async fn remove_port_mapping(
        &self,
        _external_port: u16,
        _protocol: igd::PortMappingProtocol,
    ) -> Result<(), NetworkingError> {
        // Stub implementation to avoid IGD API issues
        Ok(())
    }
}

/// Playit.gg tunnel manager.
pub struct PlayitTunnelManager {
    client: Client,
    secret_key: Option<String>,
}

impl PlayitTunnelManager {
    pub fn new(secret_key: Option<String>) -> Self {
        PlayitTunnelManager {
            client: Client::new(),
            secret_key,
        }
    }

    pub async fn create_tunnel(&self, name: &str) -> Result<String, NetworkingError> {
        let secret_key = self.secret_key.as_ref()
            .ok_or(NetworkingError::PlayitApiError("No key".into()))?;

        let response = self.client
            .post("https://api.playit.gg/v1/tunnel")
            .bearer_auth(secret_key)
            .json(&serde_json::json!({ "name": name, "port": 25565 }))
            .send()
            .await
            .map_err(|e| NetworkingError::PlayitApiError(e.to_string()))?;

        let body: serde_json::Value = response.json().await
            .map_err(|e| NetworkingError::PlayitApiError(e.to_string()))?;

        Ok(body["url"].as_str().unwrap_or("").to_string())
    }
}