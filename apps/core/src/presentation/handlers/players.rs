//! HTTP handlers for player management. Reads are served from on-disk JSON;
//! every mutation builds a sanitized RCON command and runs it against the
//! running server. Player names are validated to the Minecraft charset and
//! free-text reasons are stripped of control characters to prevent RCON
//! command injection.

use std::sync::Arc;

use axum::{
	extract::{Path, State},
	Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
	application::{
		player_service::{list_players, run_command},
		state::AppState,
	},
	presentation::{error::ApiError, extractors::AuthUser},
};

#[derive(Debug, Deserialize)]
pub struct PlayerNameRequest {
	pub name: String,
	#[serde(default)]
	pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IpRequest {
	pub ip: String,
	#[serde(default)]
	pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WhitelistToggleRequest {
	pub enabled: bool,
}

/// GET /instances/:id/players
pub async fn list_players_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	let players = list_players(&state, &id).await?;
	Ok(Json(json!(players)))
}

/// Validate a Minecraft username: 1–16 chars of `[A-Za-z0-9_]`.
fn validate_name(name: &str) -> Result<&str, ApiError> {
	let name = name.trim();
	if name.is_empty()
		|| name.len() > 16
		|| !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
	{
		return Err(ApiError::BadRequest("invalid player name".into()));
	}
	Ok(name)
}

/// Validate an IPv4/IPv6 literal.
fn validate_ip(ip: &str) -> Result<&str, ApiError> {
	let ip = ip.trim();
	if ip.parse::<std::net::IpAddr>().is_err() {
		return Err(ApiError::BadRequest("invalid ip address".into()));
	}
	Ok(ip)
}

/// Strip control characters from a free-text reason so it cannot inject a new
/// RCON command or break the line.
fn sanitize_reason(reason: &Option<String>) -> String {
	reason
		.as_deref()
		.unwrap_or("")
		.chars()
		.filter(|c| !c.is_control())
		.collect::<String>()
		.trim()
		.to_string()
}

async fn exec(
	state: &Arc<AppState>,
	id: &str,
	command: String,
) -> Result<Json<Value>, ApiError> {
	let response = run_command(state, id, command).await?;
	Ok(Json(json!({ "response": response })))
}

/// POST /instances/:id/players/kick
pub async fn kick_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<PlayerNameRequest>,
) -> Result<Json<Value>, ApiError> {
	let name = validate_name(&body.name)?;
	let reason = sanitize_reason(&body.reason);
	let command = if reason.is_empty() {
		format!("kick {name}")
	} else {
		format!("kick {name} {reason}")
	};
	exec(&state, &id, command).await
}

/// POST /instances/:id/players/op
pub async fn op_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<PlayerNameRequest>,
) -> Result<Json<Value>, ApiError> {
	let name = validate_name(&body.name)?;
	exec(&state, &id, format!("op {name}")).await
}

/// POST /instances/:id/players/deop
pub async fn deop_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<PlayerNameRequest>,
) -> Result<Json<Value>, ApiError> {
	let name = validate_name(&body.name)?;
	exec(&state, &id, format!("deop {name}")).await
}

/// POST /instances/:id/players/whitelist
pub async fn whitelist_add_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<PlayerNameRequest>,
) -> Result<Json<Value>, ApiError> {
	let name = validate_name(&body.name)?;
	exec(&state, &id, format!("whitelist add {name}")).await
}

/// DELETE /instances/:id/players/whitelist/:name
pub async fn whitelist_remove_handler(
	_auth: AuthUser,
	Path((id, name)): Path<(String, String)>,
	State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
	let name = validate_name(&name)?;
	exec(&state, &id, format!("whitelist remove {name}")).await
}

/// POST /instances/:id/players/whitelist/toggle
pub async fn whitelist_toggle_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<WhitelistToggleRequest>,
) -> Result<Json<Value>, ApiError> {
	let command = if body.enabled {
		"whitelist on"
	} else {
		"whitelist off"
	};
	exec(&state, &id, command.to_string()).await
}

/// POST /instances/:id/players/ban
pub async fn ban_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<PlayerNameRequest>,
) -> Result<Json<Value>, ApiError> {
	let name = validate_name(&body.name)?;
	let reason = sanitize_reason(&body.reason);
	let command = if reason.is_empty() {
		format!("ban {name}")
	} else {
		format!("ban {name} {reason}")
	};
	exec(&state, &id, command).await
}

/// POST /instances/:id/players/pardon
pub async fn pardon_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<PlayerNameRequest>,
) -> Result<Json<Value>, ApiError> {
	let name = validate_name(&body.name)?;
	exec(&state, &id, format!("pardon {name}")).await
}

/// POST /instances/:id/players/ban-ip
pub async fn ban_ip_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<IpRequest>,
) -> Result<Json<Value>, ApiError> {
	let ip = validate_ip(&body.ip)?;
	let reason = sanitize_reason(&body.reason);
	let command = if reason.is_empty() {
		format!("ban-ip {ip}")
	} else {
		format!("ban-ip {ip} {reason}")
	};
	exec(&state, &id, command).await
}

/// POST /instances/:id/players/pardon-ip
pub async fn pardon_ip_handler(
	_auth: AuthUser,
	Path(id): Path<String>,
	State(state): State<Arc<AppState>>,
	Json(body): Json<IpRequest>,
) -> Result<Json<Value>, ApiError> {
	let ip = validate_ip(&body.ip)?;
	exec(&state, &id, format!("pardon-ip {ip}")).await
}
