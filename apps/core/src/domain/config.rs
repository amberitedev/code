//! Domain config - Configuration setting types.
//! Defines SettingManifest and ConfigurableValue.

use serde::{Deserialize, Serialize};

/// A configurable value for UI display.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ConfigurableValue {
    String(String),
    Boolean(bool),
    UnsignedInteger(u32),
    Enum(Vec<String>),
    Number(f64),
}

/// A setting manifest describing a configuration option.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettingManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub value: Option<ConfigurableValue>,
    pub default: Option<ConfigurableValue>,
    pub requires_restart: bool,
    pub order: u32,
}

impl SettingManifest {
    pub fn string(id: &str, name: &str, description: &str) -> Self {
        SettingManifest {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            value: None,
            default: None,
            requires_restart: false,
            order: 0,
        }
    }

    pub fn boolean(id: &str, name: &str, description: &str) -> Self {
        SettingManifest {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            value: Some(ConfigurableValue::Boolean(false)),
            default: Some(ConfigurableValue::Boolean(false)),
            requires_restart: false,
            order: 0,
        }
    }

    pub fn unsigned_integer(id: &str, name: &str, description: &str) -> Self {
        SettingManifest {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            value: Some(ConfigurableValue::UnsignedInteger(0)),
            default: Some(ConfigurableValue::UnsignedInteger(0)),
            requires_restart: false,
            order: 0,
        }
    }
}
