//! A share diff as the App and Core give it to the UI, in upstream app-lib's shape
//! (`SharedInstanceUpdateDiff`), which the content diff list reads.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One change of a share diff, named for people.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SharedInstanceUpdateDiff {
    #[serde(rename = "type")]
    pub type_: SharedInstanceUpdateDiffType,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    /// A file without a Modrinth project, by its name in its type's folder.
    pub file_name: Option<String>,
    pub current_version_name: Option<String>,
    pub new_version_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub config_file_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub config_file_names: Vec<String>,
    /// A removal of something the owner had turned off, which members never ran.
    #[serde(default, skip_serializing_if = "is_false")]
    #[ts(optional, as = "Option<bool>")]
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SharedInstanceUpdateDiffType {
    Added,
    Removed,
    Updated,
    ModpackLinked,
    ModpackUpdated,
    ModpackUnlinked,
    GameVersionUpdated,
    LoaderUpdated,
    ConfigFilesUpdated,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl SharedInstanceUpdateDiff {
    /// A change with only its kind set.
    pub(super) fn of(type_: SharedInstanceUpdateDiffType) -> Self {
        Self {
            type_,
            project_id: None,
            project_name: None,
            file_name: None,
            current_version_name: None,
            new_version_name: None,
            config_file_count: None,
            config_file_names: Vec::new(),
            disabled: false,
        }
    }
}
