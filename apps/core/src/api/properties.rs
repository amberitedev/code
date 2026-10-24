//! `GET|PATCH /v1/servers/{id}/worlds/{wid}/properties`: the world's `server.properties`.
//! Mirrors `Archon.Content.v1` properties types in `packages/api-client/src/modules/archon/types.ts`.
//! Editors and the owner only: the file can hold secrets, such as a resource pack URL.

use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};

use super::{AppState, activity::Action, content::world, error::ApiError};
use crate::{
    access::{Caller, Level, activity},
    content::properties,
};

/// The properties the App has a form field for, keyed with underscores instead of dashes. Part of
/// the response of GET and the body of PATCH `.../properties`. Mirrors
/// Archon.Content.v1.KnownPropertiesFields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnownPropertiesFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_cheats: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_flight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_whitelist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_gamemode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gamemode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generate_structures: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_settings: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hardcore: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_seed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_players: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tick_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_when_empty_seconds: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_idle_timeout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_resource_pack: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_pack: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_pack_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_pack_sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation_distance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_protection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_chunk_writes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_distance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub white_list: Option<String>,
}

/// Response of GET and PATCH `.../properties`; also the `properties` of a content install and of a
/// modpack upload. Mirrors Archon.Content.v1.PropertiesFields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertiesFields {
    /// Defaulted because the App's modpack upload sends `{}` when it has no settings.
    #[serde(default)]
    pub known: KnownPropertiesFields,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<BTreeMap<String, String>>,
}

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

/// The known fields that are set, as `server.properties` keys and values.
pub fn known_pairs(known: &KnownPropertiesFields) -> Vec<(String, String)> {
    let serde_json::Value::Object(fields) =
        serde_json::to_value(known).expect("strings serialize")
    else {
        return Vec::new();
    };
    fields
        .into_iter()
        .filter_map(|(key, value)| {
            Some((key.replace('_', "-"), value.as_str()?.to_owned()))
        })
        .collect()
}

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
