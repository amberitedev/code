//! Live server query orchestration: resolves an instance's bound port and
//! performs a Minecraft Server List Ping against `127.0.0.1`, returning the
//! advertised MOTD, version, player counts and measured latency.
//!
//! Unlike RCON this needs no credentials — it speaks the public status
//! handshake every Minecraft client uses, so it works for any running server
//! regardless of whether RCON is enabled. The server must be accepting
//! connections; an offline instance yields [`QueryServiceError::NotRunning`].

use std::sync::Arc;

use serde::Serialize;

use crate::{
	application::state::AppState,
	domain::instance::{InstanceId, InstanceStatus},
	infrastructure::minecraft::server_ping::{ping, PingError},
};

const QUERY_HOST: &str = "127.0.0.1";

#[derive(Debug, thiserror::Error)]
pub enum QueryServiceError {
	#[error("instance not found")]
	NotFound,
	#[error("instance is not running")]
	NotRunning,
	#[error("ping: {0}")]
	Ping(#[from] PingError),
}

/// Public snapshot of a live server's advertised status.
#[derive(Debug, Serialize)]
pub struct ServerQuery {
	pub online: bool,
	pub version_name: String,
	pub protocol: i32,
	pub players_online: i32,
	pub players_max: i32,
	pub sample: Vec<String>,
	pub motd: String,
	pub favicon: Option<String>,
	pub latency_ms: u64,
}

async fn resolve(
	state: &Arc<AppState>,
	instance_id: &str,
) -> Result<(InstanceId, u16, InstanceStatus), QueryServiceError> {
	let uid = instance_id
		.parse::<uuid::Uuid>()
		.map_err(|_| QueryServiceError::NotFound)?;
	let iid = InstanceId(uid);
	let record = state
		.instance_store
		.get(&iid)
		.await
		.map_err(|_| QueryServiceError::NotFound)?;
	Ok((iid, record.port, record.status))
}

/// Ping a running instance and return its advertised status.
pub async fn query_instance(
	state: &Arc<AppState>,
	instance_id: &str,
) -> Result<ServerQuery, QueryServiceError> {
	let (iid, port, status) = resolve(state, instance_id).await?;

	if status != InstanceStatus::Running || !state.instances.contains_key(&iid) {
		return Err(QueryServiceError::NotRunning);
	}

	let result = ping(QUERY_HOST, port).await?;
	Ok(ServerQuery {
		online: true,
		version_name: result.version_name,
		protocol: result.protocol,
		players_online: result.players_online,
		players_max: result.players_max,
		sample: result.sample,
		motd: result.motd,
		favicon: result.favicon,
		latency_ms: result.latency_ms,
	})
}
