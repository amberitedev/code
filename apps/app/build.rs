use tauri_build::{DefaultPermissionRule, InlinedPlugin};

fn main() {
    set_env();

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
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            ),
    )
    .expect("Failed to run tauri-build");
}

/// Account code reads Modrinth's URLs at compile time, from the `.env` the engine's build reads.
fn set_env() {
    const ENGINE_ENV: &str = "../../packages/engine/.env";
    println!("cargo::rerun-if-changed={ENGINE_ENV}");
    for (name, value) in dotenvy::from_path_iter(ENGINE_ENV)
        .into_iter()
        .flatten()
        .flatten()
    {
        if name.starts_with("MODRINTH_") {
            println!("cargo::rustc-env={name}={value}");
        }
    }
}
