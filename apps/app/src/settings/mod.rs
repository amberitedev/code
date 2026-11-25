//! The desktop's settings. Reading or writing them keeps the fullscreen setting in step with
//! synced game options.

use engine::state::{Hooks, MemorySettings, WindowSize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::state::Desktop;

pub mod routes;
pub mod rows;

/// The desktop's settings, one row of app.db.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Settings {
    pub max_concurrent_downloads: usize,
    pub max_concurrent_writes: usize,

    pub theme: Theme,
    pub locale: String,
    pub default_page: DefaultPage,
    pub collapsed_navigation: bool,
    pub hide_nametag_skins_page: bool,
    pub advanced_rendering: bool,
    pub native_decorations: bool,
    pub toggle_sidebar: bool,
    pub sync_theme_across_devices: bool,
    pub sync_behavior_across_devices: bool,
    #[serde(default = "default_true")]
    pub sync_features_across_devices: bool,
    #[serde(default = "default_true")]
    pub show_files_tab_in_instances: bool,
    #[serde(default = "default_true")]
    pub show_worlds_tab_in_instances: bool,
    #[serde(default)]
    pub show_screenshots_tab_in_instances: bool,
    #[serde(default = "default_true")]
    pub show_skin_selector_in_sidebar: bool,

    #[serde(default = "default_true")]
    pub show_jump_in: bool,
    #[serde(default)]
    pub always_show_copy_details: bool,
    #[serde(default)]
    pub hide_installed_modpacks: bool,
    #[serde(default = "default_true")]
    pub advanced_filters_collapsed: bool,
    #[serde(default)]
    pub dismissed_photosensitivity_filter_warning: bool,
    #[serde(default)]
    pub friends_active_collapsed: bool,
    #[serde(default)]
    pub friends_online_collapsed: bool,
    #[serde(default = "default_true")]
    pub friends_offline_collapsed: bool,
    #[serde(default = "default_true")]
    pub friends_pending_collapsed: bool,

    pub telemetry: bool,
    pub discord_rpc: bool,
    pub personalized_ads: bool,

    pub extra_launch_args: Vec<String>,
    pub custom_env_vars: Vec<(String, String)>,
    pub memory: MemorySettings,
    pub force_fullscreen: bool,
    pub game_resolution: WindowSize,
    pub hide_on_process_start: bool,
    #[serde(default)]
    pub refocus_on_game_close: bool,
    #[serde(default)]
    pub compact_instance_cards: bool,
    #[serde(default = "default_true")]
    pub show_play_time: bool,
    #[serde(default = "default_true")]
    pub warn_on_unknown_modpacks: bool,
    #[serde(default)]
    pub skip_non_essential_warnings: bool,
    pub hooks: Hooks,

    pub custom_dir: Option<String>,
    pub prev_custom_dir: Option<String>,

    pub developer_mode: bool,
    pub feature_flags: HashMap<FeatureFlag, bool>,

    pub skipped_update: Option<String>,
    pub pending_update_toast_for_version: Option<String>,
    pub auto_download_updates: Option<bool>,

    pub version: usize,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, Hash, PartialEq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub enum FeatureFlag {
    PagePath,
    ProjectBackground,
    ServerRamAsBytesAlwaysOn,
    AlwaysShowAppControls,
    ShowSyncInstancesUpdateModal,
    PrideFundraiser,
    ServersInApp,
    ServerProjectQa,
    I18nDebug,
    LocalhostSignIn,
    ShowVersionEnvironmentColumn,
}

/// The App's theme.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS), ts(rename = "AppTheme"))]
pub enum Theme {
    Dark,
    Light,
    Oled,
    Retro,
    System,
}

impl Theme {
    pub fn as_str(&self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
            Theme::Oled => "oled",
            Theme::Retro => "retro",
            Theme::System => "system",
        }
    }

    pub fn from_string(string: &str) -> Theme {
        match string {
            "dark" => Theme::Dark,
            "light" => Theme::Light,
            "oled" => Theme::Oled,
            "retro" => Theme::Retro,
            "system" => Theme::System,
            _ => Theme::Dark,
        }
    }
}

/// Opening window to start with
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub enum DefaultPage {
    Home,
    Library,
}

impl DefaultPage {
    pub fn as_str(&self) -> &'static str {
        match self {
            DefaultPage::Home => "home",
            DefaultPage::Library => "library",
        }
    }

    pub fn from_string(string: &str) -> Self {
        match string {
            "home" => Self::Home,
            "library" => Self::Library,
            _ => Self::Home,
        }
    }
}

/// Gets entire settings
#[tracing::instrument(skip(state))]
pub async fn get(state: &Desktop) -> engine::Result<Settings> {
    let _guard = state.synced_options().lock().await;
    let mut settings = Settings::get(state.db()).await?;
    if crate::synced_options::state::game_options_sync_is_enabled(state.db())
        .await?
        && let Some(fullscreen) =
            crate::synced_options::shared_fullscreen_value(state).await?
        && settings.force_fullscreen != fullscreen
    {
        settings.force_fullscreen = fullscreen;
        settings.update(state.db()).await?;
    }
    Ok(settings)
}

/// Sets entire settings
#[tracing::instrument(skip(state))]
pub async fn set(state: &Desktop, settings: Settings) -> engine::Result<()> {
    let _guard = state.synced_options().lock().await;
    let current_settings = Settings::get(state.db()).await?;
    let game_options_sync_enabled =
        crate::synced_options::state::game_options_sync_is_enabled(state.db())
            .await?;
    let current_fullscreen = if game_options_sync_enabled {
        crate::synced_options::shared_fullscreen_value(state)
            .await?
            .unwrap_or(current_settings.force_fullscreen)
    } else {
        current_settings.force_fullscreen
    };
    let fullscreen_changed = current_fullscreen != settings.force_fullscreen;
    let shared_fullscreen_changed =
        if game_options_sync_enabled && fullscreen_changed {
            crate::synced_options::update_shared_fullscreen_from_app(
                state,
                settings.force_fullscreen,
            )
            .await?
        } else {
            false
        };
    settings.update(state.db()).await?;

    if shared_fullscreen_changed {
        let result =
            crate::synced_options::sync_all_participating_instances(state)
                .await;
        if result.failed > 0 {
            tracing::warn!(
                "Fullscreen was saved, but {} synced instance(s) could not be updated",
                result.failed
            );
        }
    }

    Ok(())
}
