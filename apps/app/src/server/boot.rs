use std::env;

use super::endpoints::boot::{Boot, Platform};
use crate::dev;

impl Boot {
    /// `version` is the App's, from its Tauri package info.
    pub fn current(version: String) -> Self {
        let os = os_info::get();
        Self {
            version,
            platform: PLATFORM,
            os_version: os.version().to_string(),
            dev: cfg!(debug_assertions),
            updates_enabled: cfg!(feature = "updater")
                && env::var("MODRINTH_EXTERNAL_UPDATE_PROVIDER").is_err(),
            disable_mouseover: cfg!(target_os = "macos")
                && !matches!(
                    os.version(),
                    os_info::Version::Semantic(major, minor, _) if (*major, *minor) >= (12, 3)
                ),
            backend_url: dev::config()
                .and_then(|config| config.backend_url.clone()),
        }
    }
}

const PLATFORM: Platform = if cfg!(target_os = "windows") {
    Platform::Windows
} else if cfg!(target_os = "macos") {
    Platform::Macos
} else {
    Platform::Linux
};
