//! Theseus instance management interface

mod content;
mod export_mrpack;
mod files;
pub(crate) use files::InstanceFileWriter;
pub use files::{
    InstanceFileItem, check_save_destination, create_instance_directory,
    delete_instance_file, list_instance_files, read_instance_file,
    rename_instance_file, save_instance_file_as, validate_instance_file_write,
    write_instance_file,
};
mod get;
mod groups;
mod icon;
mod install;
mod lifecycle;
mod paths;
mod projects;
mod run;
mod server_content;
mod shared;

pub use self::content::{
    get_content_items, get_dependencies_as_content_items,
    get_install_candidates, get_installed_project_ids,
    get_linked_modpack_content, get_linked_modpack_info, get_projects,
    list_content_sets, refresh_content_updates, sync_content_files,
};
pub use self::export_mrpack::{
    PackExportCandidate, PackExportCandidateType, create_mrpack_json,
    export_mrpack, get_pack_export_candidates,
    get_pack_export_candidates_for_parent,
};
pub use self::get::{get, get_many, list};
pub use self::groups::{
    FAVORITES_GROUP_ID, InstanceGroup, InstanceGroupMembershipUpdate,
    create_group, delete_group, list_groups, rename_group,
    set_group_memberships, set_group_order,
};
pub use self::icon::{
    cache_generated_icon, cache_icon_bytes, cache_remote_icon,
    edit_generated_icon, edit_generated_icon_if_empty, edit_icon,
};
pub(crate) use self::icon::{cache_icon, cache_icon_from_path};
pub use self::icon::{edit_icon_image, read_icon, remove_icon};
pub use self::install::get_optimal_jre_key;
pub use self::lifecycle::{create, edit, remove};
pub use self::paths::{get_full_path, get_mod_full_path};
pub use self::projects::{
    InstallProjectWithDependenciesRequest, add_project_from_path,
    add_project_from_version, ensure_metadata_content_unlocked,
    install_project_with_dependencies, is_file_on_modrinth, remove_project,
    repair_managed_modrinth, set_project_locked,
    switch_project_version_with_dependencies, toggle_disable_project,
    update_managed_modrinth_version, update_project,
};
pub use self::run::{
    LaunchCredentials, LaunchDefaults, LaunchRequest, QuickPlayType, kill,
    launch, try_update_playtime_by_instance_id,
};
pub use self::server_content::{
    ContentSide, EditedFile, complete_side_review, decide_edited, edited_files,
    place_client_only, repair_content, set_content_side,
    set_content_sides_unlocked,
};
pub(crate) use self::shared::{
    CONFIG_DIRECTORY, CONFIG_FILE_EXTENSIONS, CONFIG_FILE_TYPE,
    CONFIG_SYNC_ENABLED, PublishReports, is_shareable_file_path,
    set_applied_files, share_backend, shared_instance_update_data,
    shared_modpack_files,
};
pub use self::shared::{
    SharedHistorySettings, SharedInstanceExternalFilePreview,
    SharedInstanceHistoryEntry, SharedInstanceInstallPreview,
    SharedInstanceInvite, SharedInstanceInviteCreator,
    SharedInstanceInviteInstallPreview, SharedInstanceInviteLink,
    SharedInstanceJoinType, SharedInstancePublishFile,
    SharedInstancePublishPreview, SharedInstanceUpdateDiff,
    SharedInstanceUpdateDiffType, SharedInstanceUpdatePreview,
    SharedInstanceUser, SharedInstanceUsers,
    accept_pending_shared_instance_invite,
    accept_shared_instance_invite_for_install, cache_shared_instance_library,
    can_active_user_use_shared_instances, create_shared_instance_invite_link,
    decline_pending_shared_instance_invite, delete_shared_instance_version,
    dismiss_shared_instance_restored_notice, ensure_shared_instance,
    get_cached_shared_instance_library, get_shared_history_settings,
    get_shared_instance_history, get_shared_instance_install_preview,
    get_shared_instance_invite_install_preview, get_shared_instance_invites,
    get_shared_instance_publish_preview,
    get_shared_instance_publish_support_details,
    get_shared_instance_update_preview, get_shared_instance_users,
    install_shared_instance, invite_shared_instance_users,
    leave_remote_shared_instance, publish_shared_instance,
    remove_shared_instance_users, restore_shared_instance_version,
    revoke_shared_instance_invite, set_shared_history_kept,
    unlink_shared_instance, unpublish_remote_shared_instance,
    unpublish_shared_instance, update_shared_instance,
};
