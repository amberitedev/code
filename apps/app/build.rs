use tauri_build::{DefaultPermissionRule, InlinedPlugin};

fn main() {
    // Sadly, there is no better way to do it right now
    // You could try parsing source code here and detecting #[tauri::command]
    // But I think it's not worth it
    // https://github.com/tauri-apps/tauri/issues/10075
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .codegen(tauri_build::CodegenContext::new())
            .plugin(
                "instance",
                InlinedPlugin::new()
                    .commands(&[
                        "instance_remove",
                        "instance_get_projects",
                        "instance_get_installed_project_ids",
                        "instance_get_install_candidates",
                        "instance_content",
                        "instance_get_content_items",
                        "instance_sync_content_files",
                        "instance_refresh_content_updates",
                        "instance_get_dependencies_as_content_items",
                        "instance_get_linked_modpack_info",
                        "instance_get_linked_modpack_content",
                        "instance_get_optimal_jre_key",
                        "instance_get_full_path",
                        "instance_get_mod_full_path",
                        "instance_check_installed",
                        "instance_update_project",
                        "instance_add_project_from_version",
                        "instance_install_project_with_dependencies",
                        "instance_switch_project_version_with_dependencies",
                        "instance_add_project_from_path",
                        "instance_is_file_on_modrinth",
                        "instance_toggle_disable_project",
                        "instance_set_project_locked",
                        "instance_remove_project",
                        "instance_update_managed_modrinth_version",
                        "instance_repair_managed_modrinth",
                        "instance_run",
                        "instance_kill",
                        "instance_edit",
                        "instance_edit_icon",
                        "instance_cache_icon",
                        "instance_edit_generated_icon",
                        "instance_cache_generated_icon",
                        "instance_share_can_current_user_use",
                        "instance_share_get_users",
                        "instance_share_invite_users",
                        "instance_share_create_invite_link",
                        "instance_share_get_invites",
                        "instance_share_revoke_invite",
                        "instance_share_remove_users",
                        "instance_share_get_publish_preview",
                        "instance_share_publish",
                        "instance_share_publish_support_details",
                        "instance_share_unlink",
                        "instance_share_unpublish",
                        "instance_share_ensure",
                        "instance_share_unpublish_remote",
                        "instance_share_leave_remote",
                        "instance_share_get_cached_library",
                        "instance_share_cache_library",
                        "instance_share_dismiss_restored_notice",
                        "instance_export_mrpack",
                        "instance_get_pack_export_candidates",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            ),
    )
    .expect("Failed to run tauri-build");
}
