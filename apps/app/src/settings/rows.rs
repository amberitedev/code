//! The settings row of app.db.

use anyhow::bail;
use engine::state::{
    DefaultPage, Hooks, MemorySettings, Settings, Theme, WindowSize,
};
use sqlx::{Pool, Sqlite};

const CURRENT_VERSION: usize = 3;

pub trait SettingsRow: Sized {
    async fn get(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> engine::Result<Self>;
    async fn update(
        &self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> engine::Result<()>;
    async fn migrate(exec: &Pool<Sqlite>) -> engine::Result<()>;
    fn perform_migration(&mut self) -> engine::Result<()>;
}

impl SettingsRow for Settings {
    async fn get(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> engine::Result<Self> {
        let res = sqlx::query!(
            "
            SELECT
                max_concurrent_writes, max_concurrent_downloads,
                theme, locale, default_page, collapsed_navigation, hide_nametag_skins_page, advanced_rendering, native_decorations,
                discord_rpc, developer_mode, telemetry, personalized_ads,
                json(extra_launch_args) extra_launch_args, json(custom_env_vars) custom_env_vars,
                mc_memory_max, mc_force_fullscreen, mc_game_resolution_x, mc_game_resolution_y, hide_on_process_start,
                hook_pre_launch, hook_wrapper, hook_post_exit,
                custom_dir, prev_custom_dir, json(feature_flags) feature_flags, toggle_sidebar,
                skipped_update, pending_update_toast_for_version, auto_download_updates,
				sync_theme_across_devices, sync_behavior_across_devices, sync_features_across_devices,
				show_files_tab_in_instances, show_worlds_tab_in_instances,
				show_screenshots_tab_in_instances, show_skin_selector_in_sidebar,
				refocus_on_game_close, compact_instance_cards, show_play_time, warn_on_unknown_modpacks, skip_non_essential_warnings,
				show_jump_in,
				always_show_copy_details,
				hide_installed_modpacks,
				advanced_filters_collapsed,
				dismissed_photosensitivity_filter_warning,
				friends_active_collapsed,
				friends_online_collapsed,
				friends_offline_collapsed,
				friends_pending_collapsed,
                version
            FROM settings
            "
        )
            .fetch_one(exec)
            .await?;

        Ok(Self {
            max_concurrent_downloads: res.max_concurrent_downloads as usize,
            max_concurrent_writes: res.max_concurrent_writes as usize,
            theme: Theme::from_string(&res.theme),
            locale: res.locale,
            default_page: DefaultPage::from_string(&res.default_page),
            collapsed_navigation: res.collapsed_navigation == 1,
            hide_nametag_skins_page: res.hide_nametag_skins_page == 1,
            advanced_rendering: res.advanced_rendering == 1,
            native_decorations: res.native_decorations == 1,
            toggle_sidebar: res.toggle_sidebar == 1,
            show_jump_in: res.show_jump_in == 1,
            always_show_copy_details: res.always_show_copy_details == 1,
            hide_installed_modpacks: res.hide_installed_modpacks == 1,
            advanced_filters_collapsed: res.advanced_filters_collapsed == 1,
            dismissed_photosensitivity_filter_warning: res
                .dismissed_photosensitivity_filter_warning
                == 1,
            friends_active_collapsed: res.friends_active_collapsed == 1,
            friends_online_collapsed: res.friends_online_collapsed == 1,
            friends_offline_collapsed: res.friends_offline_collapsed == 1,
            friends_pending_collapsed: res.friends_pending_collapsed == 1,
            telemetry: res.telemetry == 1,
            discord_rpc: res.discord_rpc == 1,
            developer_mode: res.developer_mode == 1,
            personalized_ads: res.personalized_ads == 1,
            extra_launch_args: res
                .extra_launch_args
                .as_ref()
                .and_then(|x| serde_json::from_str(x).ok())
                .unwrap_or_default(),
            custom_env_vars: res
                .custom_env_vars
                .as_ref()
                .and_then(|x| serde_json::from_str(x).ok())
                .unwrap_or_default(),
            memory: MemorySettings {
                maximum: res.mc_memory_max as u32,
            },
            force_fullscreen: res.mc_force_fullscreen == 1,
            game_resolution: WindowSize(
                res.mc_game_resolution_x as u16,
                res.mc_game_resolution_y as u16,
            ),
            hide_on_process_start: res.hide_on_process_start == 1,
            refocus_on_game_close: res.refocus_on_game_close == 1,
            compact_instance_cards: res.compact_instance_cards == 1,
            show_play_time: res.show_play_time == 1,
            warn_on_unknown_modpacks: res.warn_on_unknown_modpacks == 1,
            skip_non_essential_warnings: res.skip_non_essential_warnings == 1,
            hooks: Hooks {
                pre_launch: res.hook_pre_launch,
                wrapper: res.hook_wrapper,
                post_exit: res.hook_post_exit,
            },
            custom_dir: res.custom_dir,
            prev_custom_dir: res.prev_custom_dir,
            feature_flags: res
                .feature_flags
                .as_ref()
                .and_then(|x| serde_json::from_str(x).ok())
                .unwrap_or_default(),
            skipped_update: res.skipped_update,
            pending_update_toast_for_version: res
                .pending_update_toast_for_version,
            auto_download_updates: res.auto_download_updates.map(|x| x == 1),
            sync_theme_across_devices: res.sync_theme_across_devices == 1,
            sync_behavior_across_devices: res.sync_behavior_across_devices == 1,
            sync_features_across_devices: res.sync_features_across_devices == 1,
            show_files_tab_in_instances: res.show_files_tab_in_instances == 1,
            show_worlds_tab_in_instances: res.show_worlds_tab_in_instances == 1,
            show_screenshots_tab_in_instances: res
                .show_screenshots_tab_in_instances
                == 1,
            show_skin_selector_in_sidebar: res.show_skin_selector_in_sidebar
                == 1,
            version: res.version as usize,
        })
    }

    async fn update(
        &self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> engine::Result<()> {
        let max_concurrent_writes = self.max_concurrent_writes as i32;
        let max_concurrent_downloads = self.max_concurrent_downloads as i32;
        let theme = self.theme.as_str();
        let default_page = self.default_page.as_str();
        let extra_launch_args = serde_json::to_string(&self.extra_launch_args)?;
        let custom_env_vars = serde_json::to_string(&self.custom_env_vars)?;
        let feature_flags = serde_json::to_string(&self.feature_flags)?;
        let version = self.version as i64;

        sqlx::query!(
            "
            UPDATE settings
            SET
                max_concurrent_writes = $1,
                max_concurrent_downloads = $2,

                theme = $3,
                locale = $4,
                default_page = $5,
                collapsed_navigation = $6,
                advanced_rendering = $7,
                native_decorations = $8,

                discord_rpc = $9,
                developer_mode = $10,
                telemetry = $11,
                personalized_ads = $12,

                extra_launch_args = jsonb($13),
                custom_env_vars = jsonb($14),
                mc_memory_max = $15,
                mc_force_fullscreen = $16,
                mc_game_resolution_x = $17,
                mc_game_resolution_y = $18,
                hide_on_process_start = $19,

                hook_pre_launch = $20,
                hook_wrapper = $21,
                hook_post_exit = $22,

                custom_dir = $23,
                prev_custom_dir = $24,

                toggle_sidebar = $25,
                feature_flags = $26,
                hide_nametag_skins_page = $27,

                skipped_update = $28,
                pending_update_toast_for_version = $29,
                auto_download_updates = $30,

                sync_theme_across_devices = $31,
                sync_behavior_across_devices = $32,
				sync_features_across_devices = $33,
				show_files_tab_in_instances = $34,
				show_worlds_tab_in_instances = $35,
				show_screenshots_tab_in_instances = $36,
				show_skin_selector_in_sidebar = $37,

				version = $38,
				refocus_on_game_close = $39,
				compact_instance_cards = $40,
				show_play_time = $41,
				warn_on_unknown_modpacks = $42,
				skip_non_essential_warnings = $43,
				show_jump_in = $44,
				always_show_copy_details = $45,
				hide_installed_modpacks = $46,
				advanced_filters_collapsed = $47,
				dismissed_photosensitivity_filter_warning = $48,
				friends_active_collapsed = $49,
				friends_online_collapsed = $50,
				friends_offline_collapsed = $51,
				friends_pending_collapsed = $52
            ",
            max_concurrent_writes,
            max_concurrent_downloads,
            theme,
            self.locale,
            default_page,
            self.collapsed_navigation,
            self.advanced_rendering,
            self.native_decorations,
            self.discord_rpc,
            self.developer_mode,
            self.telemetry,
            self.personalized_ads,
            extra_launch_args,
            custom_env_vars,
            self.memory.maximum,
            self.force_fullscreen,
            self.game_resolution.0,
            self.game_resolution.1,
            self.hide_on_process_start,
            self.hooks.pre_launch,
            self.hooks.wrapper,
            self.hooks.post_exit,
            self.custom_dir,
            self.prev_custom_dir,
            self.toggle_sidebar,
            feature_flags,
            self.hide_nametag_skins_page,
            self.skipped_update,
            self.pending_update_toast_for_version,
            self.auto_download_updates,
            self.sync_theme_across_devices,
            self.sync_behavior_across_devices,
            self.sync_features_across_devices,
            self.show_files_tab_in_instances,
            self.show_worlds_tab_in_instances,
            self.show_screenshots_tab_in_instances,
            self.show_skin_selector_in_sidebar,
            version,
            self.refocus_on_game_close,
            self.compact_instance_cards,
            self.show_play_time,
            self.warn_on_unknown_modpacks,
            self.skip_non_essential_warnings,
            self.show_jump_in,
            self.always_show_copy_details,
            self.hide_installed_modpacks,
            self.advanced_filters_collapsed,
            self.dismissed_photosensitivity_filter_warning,
            self.friends_active_collapsed,
            self.friends_online_collapsed,
            self.friends_offline_collapsed,
            self.friends_pending_collapsed,
        )
        .execute(exec)
        .await?;

        Ok(())
    }

    async fn migrate(exec: &Pool<Sqlite>) -> engine::Result<()> {
        let mut settings = Self::get(exec).await?;

        if settings.version < CURRENT_VERSION {
            tracing::info!(
                "Migrating settings version {} to {:?}",
                settings.version,
                CURRENT_VERSION
            );
        }
        while settings.version < CURRENT_VERSION {
            if let Err(err) = settings.perform_migration() {
                tracing::error!(
                    "Failed to migrate settings from version {}: {}",
                    settings.version,
                    err
                );
                return Err(err);
            }
        }

        settings.update(exec).await?;

        Ok(())
    }

    fn perform_migration(&mut self) -> engine::Result<()> {
        match self.version {
            1 => {
                let quoter = shlex::Quoter::new().allow_nul(true);

                // Previously split by spaces
                if let Some(pre_launch) = self.hooks.pre_launch.as_ref() {
                    self.hooks.pre_launch =
                        Some(quoter.join(pre_launch.split(' ')).unwrap())
                }

                // Previously treated as complete path to command
                if let Some(wrapper) = self.hooks.wrapper.as_ref() {
                    self.hooks.wrapper =
                        Some(quoter.quote(wrapper).unwrap().to_string())
                }

                // Previously split by spaces
                if let Some(post_exit) = self.hooks.post_exit.as_ref() {
                    self.hooks.post_exit =
                        Some(quoter.join(post_exit.split(' ')).unwrap())
                }

                self.version = 2;
            }
            2 => {
                // Update old default memory setting from 2GB to 4GB (depending on system memory)
                const LEGACY_DEFAULT_MEMORY_MB: u32 = 2048;
                if self.memory.maximum == LEGACY_DEFAULT_MEMORY_MB {
                    self.memory.maximum = engine::jre::default_memory_max_mb();
                }

                self.version = 3;
            }
            version => bail!("invalid settings version {version}"),
        }

        Ok(())
    }
}
