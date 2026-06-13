//! Player management: reads an instance's whitelist / ops / ban lists from the
//! Minecraft JSON files on disk (works offline) and performs mutations through
//! RCON against the running server.
//!
//! Mutations are RCON-only by design: a running Minecraft server is the single
//! source of truth for these lists and rewrites the JSON files itself. Editing
//! the files while the server runs would be silently ignored until a reload, so
//! we route every change (`whitelist add`, `op`, `ban`, `kick`, …) through RCON
//! and let the server own the files.

use std::{path::Path, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{
    application::{
        rcon_service::{execute_command, RconServiceError},
        state::AppState,
    },
    domain::instance::{InstanceId, InstanceStatus},
    infrastructure::minecraft::server_properties::read_properties,
};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WhitelistEntry {
    pub uuid: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OpEntry {
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub level: u8,
    #[serde(rename = "bypassesPlayerLimit", default)]
    pub bypasses_player_limit: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BanEntry {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub expires: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct IpBanEntry {
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub expires: String,
    #[serde(default)]
    pub reason: String,
}

/// Full snapshot of player-management state for an instance.
#[derive(Debug, Serialize)]
pub struct PlayersResponse {
    /// Names of players currently online (empty if the server is offline).
    pub online: Vec<String>,
    pub whitelist: Vec<WhitelistEntry>,
    pub ops: Vec<OpEntry>,
    pub banned_players: Vec<BanEntry>,
    pub banned_ips: Vec<IpBanEntry>,
    /// Whether `white-list=true` in `server.properties`.
    pub whitelist_enabled: bool,
    /// Whether the server is currently running (mutations available).
    pub running: bool,
}

async fn resolve(
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

/// Read and parse a JSON list file, returning an empty vec if it is missing or
/// malformed (a fresh server has not written these files yet).
async fn read_json_list<T: for<'de> Deserialize<'de>>(
    data_dir: &str,
    filename: &str,
) -> Vec<T> {
    let path = Path::new(data_dir).join(filename);
    let Ok(content) = tokio::fs::read_to_string(&path).await else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Build a full player-management snapshot for an instance.
pub async fn list_players(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<PlayersResponse, RconServiceError> {
    let (iid, data_dir, status) = resolve(state, instance_id).await?;
    let running =
        status == InstanceStatus::Running && state.instances.contains_key(&iid);

    let whitelist = read_json_list(&data_dir, "whitelist.json").await;
    let ops = read_json_list(&data_dir, "ops.json").await;
    let banned_players = read_json_list(&data_dir, "banned-players.json").await;
    let banned_ips = read_json_list(&data_dir, "banned-ips.json").await;

    let props = read_properties(Path::new(&data_dir))
        .await
        .unwrap_or_default();
    let whitelist_enabled = props
        .get("white-list")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let online = if running {
        match execute_command(state, instance_id, "list").await {
            Ok(line) => parse_online_players(&line),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };

    Ok(PlayersResponse {
        online,
        whitelist,
        ops,
        banned_players,
        banned_ips,
        whitelist_enabled,
        running,
    })
}

/// Parse the player names from a vanilla `list` response, e.g.
/// `There are 2 of a max of 20 players online: alice, bob`.
fn parse_online_players(line: &str) -> Vec<String> {
    let Some((_, rest)) = line.split_once("online:") else {
        return Vec::new();
    };
    rest.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Run a player-management command through RCON and return the server reply.
pub async fn run_command(
    state: &Arc<AppState>,
    instance_id: &str,
    command: String,
) -> Result<String, RconServiceError> {
    execute_command(state, instance_id, &command).await
}

#[cfg(test)]
mod tests {
    use super::parse_online_players;

    #[test]
    fn parses_multiple_players() {
        assert_eq!(
            parse_online_players(
                "There are 2 of a max of 20 players online: alice, bob"
            ),
            vec!["alice".to_string(), "bob".to_string()]
        );
    }

    #[test]
    fn parses_empty_server() {
        assert!(parse_online_players(
            "There are 0 of a max of 20 players online:"
        )
        .is_empty());
    }

    #[test]
    fn handles_unrelated_line() {
        assert!(parse_online_players("Unknown command").is_empty());
    }
}
