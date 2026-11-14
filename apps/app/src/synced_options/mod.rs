mod options;
mod packs;
pub mod routes;
mod servers;
pub mod state;

pub use self::options::game_options::queue_game_locale_index;
#[cfg(test)]
pub use self::options::game_options::{
    CanonicalValue as GameOptionCanonicalValue, EditableGameSetting,
    GameOptionCompatibility, GameOptionCompatibilityBucket,
    GameOptionCompatibilityReason, GameOptionCompatibilityStatus,
    GameOptionEditorChoice, GameOptionEditorDefinition, GameOptionKind,
    GameOptionMappingKind, GameOptionValidationIssue, GameOptionValueState,
    GameOptionsSourceIssue, GameSettingCategory, GameSettingChange,
    GameSettingLocaleLabel, GameSettingSource, GameSettingSourceProject,
};
pub(crate) use self::options::game_options::{
    GameLocaleIndexer, shared_fullscreen_value,
    sync_all_participating_instances, update_shared_fullscreen_from_app,
};
pub use self::options::game_options::{
    GameOptionsSourceCandidate, GameSettingLocaleLabels,
    GameSettingsEditorState, SaveGameSettingsResult, UpdateGameSettingsRequest,
    apply_launcher_overrides as apply_game_options_launcher_overrides,
    capture_pack_base as capture_game_options_pack_base,
    get_config as get_synced_game_options_config,
    get_game_setting_locale_labels,
    get_local_config as get_local_game_options_config,
    list_sync_sources as list_game_options_sync_sources,
    preview_changes as preview_synced_game_option_changes,
    preview_local_changes as preview_local_game_option_changes,
    save_changes as save_synced_game_option_changes,
    save_local_changes as save_local_game_option_changes,
    sync_before_launch as sync_game_options_before_launch,
};
pub use self::options::{
    GlobalSyncedOptions, SyncedOptionJoinPreview, SyncedOptionJoinResolution,
    SyncedOptionsOverview, get_command_history,
    get_global_options as get_global_synced_options,
    get_initialized_options as get_initialized_synced_options,
    get_instance_option_join_preview as get_synced_option_join_preview,
    get_overview as get_synced_options_overview, get_synced_options_folder,
    set_command_history, set_global_option as set_global_synced_option,
};
#[cfg(test)]
pub use self::options::{SyncedOptionCapability, SyncedOptionJoinAction};
pub use self::options::{
    prepare_instance_update, reconcile_all as reconcile_all_synced_options,
    reconcile_changed_file as reconcile_synced_option_file,
    reconcile_instance as reconcile_instance_synced_options,
    reconcile_instance_after_pack_update, remove_generated_instance_files,
    set_synced_option, start as start_synced_options,
};
#[cfg(test)]
pub use self::packs::PackSyncTarget;
pub use self::servers::{
    SyncedServer, capture_modpack_server_override, desync_server,
    discard_modpack_servers, list_synced_servers, remove_synced_server,
    update_synced_server,
};
pub(crate) use self::servers::{
    add_user_server, ensure_managed_server, list_server_records,
    remove_managed_servers, remove_server_by_index, update_server_by_index,
};

pub(crate) use self::packs::PackSyncWorker;
pub use self::packs::flush as reconcile_synced_packs;
pub use self::packs::{
    PackSyncPreview, desync_pack, get_pack_sync_preview, list_synced_packs,
    queue_reconciliation as queue_synced_pack_reconciliation,
    reconcile_after_content_change as reconcile_synced_packs_after_content_change,
    remove_synced_pack, set_synced_pack_enabled, sync_pack, upload_synced_pack,
};
pub(crate) use self::packs::{decorate_content, is_pack_path, sync_new_pack};
