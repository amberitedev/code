use contracts::{Empty, Json};
use engine::data::Settings;
use serde::{Deserialize, Serialize};

contracts::endpoints! {
    group settings;

    GetSettings: Get "/app/settings", None, (), (), Empty => Json<AppSettings>;
    /// Replaces every setting.
    SetSettings: Put "/app/settings", None, (), (), Json<AppSettings> => Empty;
    /// Keeps the previous data folder after a move failed. It answers while the engine cannot start.
    CancelDirectoryChange: Delete "/app/settings/directory-change", None, (), (), Empty => Empty;
}

/// Named the engine's `Settings` in TS, so the frontend has one settings type.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS), ts(rename = "Settings"))]
pub struct AppSettings(
    #[cfg_attr(feature = "export-ts", ts(type = "Settings"))] pub Settings,
);
