//! `GET|PATCH /v1/servers/{id}/worlds/{wid}/properties`: the world's `server.properties`.
//! Mirrors `Archon.Content.v1` properties types in `packages/api-client/src/modules/archon/types.ts`.
//! Editors and the owner only: the file can hold secrets, such as a resource pack URL.

use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;

use super::{AppState, activity::Action, content::world, error::ApiError};
use crate::{
    access::{Caller, Level, activity},
    state::servers::adapters::filesystem::properties,
};

// The settings a content install takes, as `state::servers::commands` read them.
pub use crate::state::servers::model::{
    KnownPropertiesFields, PropertiesFields, known_pairs,
};

/// Body of PATCH `.../properties`. A `null` custom value removes the key. Mirrors
/// Archon.Content.v1.PatchPropertiesFields.
#[derive(Debug, Clone, Deserialize)]
pub struct PatchPropertiesFields {
    #[serde(default)]
    pub known: KnownPropertiesFields,
    #[serde(default)]
    pub custom: BTreeMap<String, Option<String>>,
}

/// The `server.properties` keys of the known fields, with underscores where the file has dashes.
const KNOWN: [&str; 25] = [
    "allow_cheats",
    "allow_flight",
    "difficulty",
    "enforce_whitelist",
    "force_gamemode",
    "gamemode",
    "generate_structures",
    "generator_settings",
    "hardcore",
    "level_seed",
    "level_type",
    "max_players",
    "max_tick_time",
    "motd",
    "pause_when_empty_seconds",
    "player_idle_timeout",
    "require_resource_pack",
    "resource_pack",
    "resource_pack_id",
    "resource_pack_sha1",
    "simulation_distance",
    "spawn_protection",
    "sync_chunk_writes",
    "view_distance",
    "white_list",
];

/// Every key of the file, split into the known fields and the rest.
fn fields(all: BTreeMap<String, String>) -> PropertiesFields {
    let mut known = serde_json::Map::new();
    let mut custom = BTreeMap::new();
    for (key, value) in all {
        let field = key.replace('-', "_");
        if KNOWN.contains(&field.as_str()) {
            known.insert(field, serde_json::Value::String(value));
        } else {
            custom.insert(key, value);
        }
    }
    PropertiesFields {
        known: serde_json::from_value(serde_json::Value::Object(known))
            .unwrap_or_default(),
        custom: Some(custom),
    }
}

/// `GET .../properties`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<PropertiesFields>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let dir = state.servers.paths().server(&id);
    Ok(Json(fields(properties::read(&dir)?)))
}

/// `PATCH .../properties`: set the known fields given and each custom key, removing custom keys
/// set to null. Takes effect at the next start. Answers with the file as it is now.
pub async fn patch(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<PatchPropertiesFields>,
) -> Result<Json<PropertiesFields>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let dir = state.servers.paths().server(&id);
    let mut changes: BTreeMap<String, Option<String>> =
        known_pairs(&body.known)
            .into_iter()
            .map(|(key, value)| (key, Some(value)))
            .collect();
    changes.extend(body.custom);
    // Read, patched, and written under the lock installs and file edits take, so neither lands
    // between the read and the write and is lost.
    let busy = state.content.busy(&id)?;
    properties::patch(&dir, &changes)?;
    drop(busy);
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(fields(properties::read(&dir)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_keys_have_dashes_in_the_file() {
        let known = KnownPropertiesFields {
            max_players: Some("10".into()),
            white_list: Some("true".into()),
            ..Default::default()
        };
        assert_eq!(
            known_pairs(&known),
            [
                ("max-players".to_owned(), "10".to_owned()),
                ("white-list".to_owned(), "true".to_owned())
            ]
        );
        let split = fields(BTreeMap::from([
            ("max-players".into(), "10".into()),
            ("rcon.port".into(), "1".into()),
        ]));
        assert_eq!(split.known.max_players.as_deref(), Some("10"));
        assert_eq!(
            split.custom.unwrap(),
            BTreeMap::from([("rcon.port".to_owned(), "1".to_owned())])
        );
    }
}
