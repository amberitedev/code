//! The `server.properties` settings the App sends with a content install or a modpack upload,
//! and reads and changes through `api::properties`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
