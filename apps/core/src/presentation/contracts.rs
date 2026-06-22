//! Public HTTP contracts owned by Core's presentation boundary.

use serde::{Deserialize, Serialize};

pub const HANDSHAKE_PROTOCOL: u16 = 1;

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionHandshakeRequest {
    pub nonce: String,
    pub protocol: u16,
    pub known_core_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionHandshakeResponse {
    pub nonce: String,
    pub ok: bool,
    pub core_id: String,
    pub protocol: u16,
    pub version: &'static str,
    pub reason: Option<ConnectionRejectReason>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectionRejectReason {
    ProtocolMismatch,
    WrongCore,
}
