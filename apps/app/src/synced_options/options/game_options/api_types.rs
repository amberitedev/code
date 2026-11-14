use crate::synced_options::state::{CanonicalValue, GameOptionKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "GameOptionsSourceDisabledReason"))]
#[serde(rename_all = "snake_case")]
pub enum GameOptionsSourceIssue {
    InstallingOrUpdating,
    UnsupportedVersion,
    MissingOptionsFile,
    NoSyncableSettings,
    UnreadableOptionsFile,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GameOptionsSourceCandidate {
    pub source_id: String,
    pub instance_id: String,
    pub name: String,
    pub icon_path: Option<String>,
    pub game_version: Option<String>,
    pub eligible: bool,
    pub disabled_reason: Option<GameOptionsSourceIssue>,
    pub recognized_setting_count: u32,
    pub custom_setting_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GameSettingCategory {
    pub id: String,
    pub is_custom: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "GameOptionEditor"))]
pub struct GameOptionEditorDefinition {
    #[serde(rename = "type")]
    #[cfg_attr(
        test,
        ts(
            type = "\"bool\" | \"integer\" | \"decimal\" | \"enum\" | \"text\" | \"string_list\" | \"key_binding\" | \"external_raw\""
        )
    )]
    pub type_: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub unit: Option<String>,
    pub choices: Vec<GameOptionEditorChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GameOptionEditorChoice {
    pub value: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum GameOptionCompatibilityStatus {
    Ready,
    Deferred,
    WaitingForFile,
    WaitingForBase,
    NotAvailable,
    UnsupportedValue,
    Unmappable,
    CatalogUncovered,
    Controlled,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum GameOptionMappingKind {
    Direct,
    Legacy,
    Migrated,
    Lossless,
    Lossy,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum GameOptionCompatibilityReason {
    LauncherControlled,
    CatalogUncovered,
    InspectionFailed,
    WaitingForOptionsFile,
    UnsupportedValue,
    MigratesOnWrite,
    WaitingForCompatibleBase,
    MissingSetting,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GameOptionCompatibilityBucket {
    pub instance_count: u32,
    pub write_keys: Vec<String>,
    pub eventual_keys: Vec<String>,
    pub game_versions: Vec<String>,
    pub status: GameOptionCompatibilityStatus,
    pub mapping: Option<GameOptionMappingKind>,
    pub reason: Option<GameOptionCompatibilityReason>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "GameOptionCompatibilitySummary"))]
pub struct GameOptionCompatibility {
    pub total_participating: u32,
    pub will_receive: u32,
    pub write_now: u32,
    pub left_local: u32,
    pub buckets: Vec<GameOptionCompatibilityBucket>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum GameOptionValueState {
    Canonical,
    UniformLocal,
    Mixed,
    Unset,
    Invalid,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "GameOptionValidationError"))]
#[serde(rename_all = "snake_case")]
pub enum GameOptionValidationIssue {
    MissingValue,
    NoCompatibleInstances,
    InvalidValue,
    ChangedSinceOpened,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct EditableGameSetting {
    pub option_id: String,
    pub category_id: String,
    pub kind: GameOptionKind,
    pub raw_key: Option<String>,
    pub sync_enabled: bool,
    pub canonical_value: Option<CanonicalValue>,
    pub value_state: GameOptionValueState,
    pub option_revision: u64,
    pub editor: GameOptionEditorDefinition,
    pub compatibility: GameOptionCompatibility,
    pub validation_error: Option<GameOptionValidationIssue>,
    pub controlled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GameSettingsEditorState {
    pub summary_revision: String,
    pub canonical_revision: u64,
    pub catalog_revision: u32,
    pub total_participating: u32,
    pub categories: Vec<GameSettingCategory>,
    pub settings: Vec<EditableGameSetting>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct UpdateGameSettingsRequest {
    pub expected_summary_revision: String,
    pub expected_canonical_revision: u64,
    pub expected_catalog_revision: u32,
    pub changes: Vec<GameSettingChange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GameSettingChange {
    pub option_id: String,
    pub base_option_revision: u64,
    #[cfg_attr(test, ts(optional))]
    pub sync_enabled: Option<bool>,
    #[cfg_attr(test, ts(as = "Option<Option<CanonicalValue>>", optional))]
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub canonical_value: Option<Option<CanonicalValue>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SaveGameSettingsResult {
    pub state: Option<GameSettingsEditorState>,
    pub applied: u32,
    pub migrated: u32,
    pub deferred: u32,
    pub unsupported: u32,
    pub failed: u32,
    pub conflicts: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SyncOutcome {
    Applied,
    Migrated,
    Unchanged,
    Deferred,
    WaitingForFile,
}

pub(super) fn canonical_values_equal(
    left: &CanonicalValue,
    right: &CanonicalValue,
) -> bool {
    match (left, right) {
        (CanonicalValue::Decimal(left), CanonicalValue::Decimal(right)) => {
            left.parse::<f64>().ok() == right.parse::<f64>().ok()
        }
        _ => left == right,
    }
}
