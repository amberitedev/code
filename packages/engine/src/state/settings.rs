//! Launch settings the App and instances share.

use serde::{Deserialize, Serialize};

/// Minecraft memory settings
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct MemorySettings {
    pub maximum: u32,
}

/// Game window size
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct WindowSize(pub u16, pub u16);

/// Game initialization hooks
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde_with::serde_as]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Hooks {
    #[serde_as(as = "serde_with::NoneAsEmptyString")]
    #[cfg_attr(feature = "export-ts", ts(as = "String"))]
    pub pre_launch: Option<String>,
    #[serde_as(as = "serde_with::NoneAsEmptyString")]
    #[cfg_attr(feature = "export-ts", ts(as = "String"))]
    pub wrapper: Option<String>,
    #[serde_as(as = "serde_with::NoneAsEmptyString")]
    #[cfg_attr(feature = "export-ts", ts(as = "String"))]
    pub post_exit: Option<String>,
}
