use contracts::{Empty, Json};
use serde::Serialize;

contracts::endpoints! {
    group boot;

    /// What the frontend reads before it loads, none of which waits on the engine.
    GetBoot: Get "/app/boot", None, (), (), Empty => Json<Boot>;
}

#[derive(Clone, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Boot {
    pub version: String,
    pub platform: Platform,
    pub os_version: String,
    /// A debug build.
    pub dev: bool,
    pub updates_enabled: bool,
    /// Hover styles crash older macOS WebKit.
    pub disable_mouseover: bool,
    /// The dev backend a debug build's dev config names.
    pub backend_url: Option<String>,
}

/// The values `@tauri-apps/plugin-os`'s `platform()` gives on the desktops the App ships for.
#[derive(Clone, Copy, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}
