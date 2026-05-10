//! Tunnel management — Playit.gg agent + Cloudflare DNS CNAME (V2 feature).
//! V1 ships without tunnel. This module provides stubs callable from the frontend.

use crate::error::{AmberiteError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelStatus {
    pub connected: bool,
    pub url: Option<String>,
}

/// Provision and start the Playit.gg tunnel + Cloudflare DNS CNAME.
/// V1 STUB — returns an error directing users to configure port forwarding manually.
pub async fn setup_tunnel(_server_name: &str) -> Result<TunnelStatus> {
    Err(AmberiteError::Tunnel(
        "Tunnel feature is not available in V1. Configure port forwarding manually or wait for V2.".to_string(),
    ))
}

/// Get the current tunnel status.
pub async fn get_tunnel_status() -> Result<TunnelStatus> {
    Ok(TunnelStatus {
        connected: false,
        url: None,
    })
}

/// Stop the tunnel agent.
pub async fn stop_tunnel() -> Result<()> {
    // No tunnel running in V1
    Ok(())
}
