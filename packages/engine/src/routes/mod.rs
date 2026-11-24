//! The engine's HTTP API. Each handler is a `pub fn`, so a host can mount one itself, wrapped in
//! its own hook. A host's state gives the engine and its [`Access`].

mod cache;
mod content;
mod files;
mod groups;
mod install;
mod instances;
mod jre;
mod lifecycle;
mod loading;
mod logs;
mod metadata;
mod process;
mod sharing;
mod store;
mod tags;
#[cfg(test)]
mod tests;
mod worlds;

use std::sync::Arc;

use axum::{Router, extract::FromRef};
use contracts::{ApiError, ErrorCode, server::Mount};
use serde::Deserialize;

pub use self::{
    cache::*, content::*, files::*, groups::*, install::*, instances::*,
    jre::*, lifecycle::*, loading::*, logs::*, metadata::*, process::*,
    sharing::*, store::*, tags::*, worlds::*,
};
use crate::{
    Engine,
    access::Access,
    endpoints::{
        cache::{
            GetCachedOrganization, GetCachedProject, GetCachedProjectV3,
            GetCachedProjectVersions, GetCachedProjects, GetCachedProjectsV3,
            GetCachedSearchResults, GetCachedSearchResultsV3, GetCachedTeam,
            GetCachedUser, GetCachedUsers, GetCachedVersion, GetCachedVersions,
            PurgeCache,
        },
        content::{
            AddProjectFromVersion, CheckProjectInstalled, GetLinkedModpack,
            InstallModpack, InstallProjectWithDependencies, ListContentItems,
            ListDependencyContentItems, ListInstallCandidates,
            ListInstalledProjectIds, ListInstanceProjects,
            ListLinkedModpackContent, ListPackExportCandidates,
            RefreshContentUpdates, RemoveProject, RepairModpack,
            SetProjectLocked, SwitchProjectVersion, SyncContentFiles,
            ToggleProject, UnlinkModpack, UpdateModpackVersion, UpdateProject,
        },
        content::{
            CompleteSideReview, DecideEdited, EditedFiles, PlaceClientOnly,
            RepairContent, SetSide, SetSidesUnlocked,
        },
        files::{
            AddInstanceUploadFile, CancelInstanceUpload,
            CreateInstanceDirectory, DeleteInstanceFile, ExtractInstanceZip,
            FinalizeInstanceUpload, GetInstanceIcon, GetInstanceUpload,
            ListInstanceFiles, MoveInstanceFile, OpenInstanceUpload,
            ReadInstanceFile, RemoveInstanceIcon, SetEditedInstanceIcon,
            SetInstanceIcon, StatInstanceFile, WriteInstanceFile,
            ZipInstanceFiles,
        },
        groups::{
            CreateInstanceGroup, DeleteInstanceGroup, ListInstanceGroups,
            RenameInstanceGroup, SetInstanceGroupMemberships,
            SetInstanceGroupOrder,
        },
        install::{
            AcceptPendingSharedInstanceInvite, AcceptSharedInstanceInvite,
            CancelInstallJob, DeleteSharedInstanceVersion, DismissInstallJob,
            DuplicateInstance, GetInstallJob, GetInstallJobSupportDetails,
            GetSharedHistorySettings, GetSharedInstanceHistory,
            GetSharedInstanceInstallPreview, GetSharedInstanceInvitePreview,
            GetSharedInstanceUpdatePreview, InstallInstance,
            InstallSharedInstance, ListInstallJobs, PauseInstallJob,
            RestoreSharedInstanceVersion, ResumeInstallJob, RetryInstallJob,
            SetSharedHistoryKept, UpdateInstanceContent, UpdateSharedInstance,
        },
        instances::{
            EditInstance, GetInstance, GetInstances, GetOptimalJava,
            KillInstance, ListInstances, RemoveInstance,
        },
        jre::{
            FindJavaInstalls, GetJavaVersions, GetSystemMemory, InstallJava,
            SetJavaVersion,
        },
        loading::ListLoadingBars,
        logs::{
            ClearLiveLog, DeleteInstanceLog, GetInstanceLogOutput, GetLiveLog,
            ListInstanceLogs,
        },
        metadata::GetLoaderManifest,
        process::{KillProcess, ListInstanceProcesses, ListProcesses},
        sharing::{
            CreateSharedInstanceInviteLink,
            DismissSharedInstanceRestoredNotice, EnsureSharedInstance,
            GetSharedInstancePublishPreview,
            GetSharedInstancePublishSupportDetails, GetSharedInstanceUsers,
            InviteSharedInstanceUsers, LeaveRemoteSharedInstance,
            ListSharedInstanceInvites, RemoveSharedInstanceUsers,
            RevokeSharedInstanceInvite, UnlinkSharedInstance,
            UnpublishRemoteSharedInstance, UnpublishSharedInstance,
        },
        store::{CleanStore, GetStoreUsage, SetStoreCacheLimit},
        tags::{GetCategoryTags, GetGameVersionTags, GetLoaderTags},
        worlds::{
            AddInstanceServer, BackupWorld, DeleteWorld, DesyncInstanceServer,
            EditInstanceServer, EnsureManagedInstanceServer,
            GetInstanceProtocolVersion, GetServerStatus, GetSingleplayerWorld,
            ListInstanceWorlds, ListRecentWorlds, RemoveInstanceServer,
            RenameWorld, ResetWorldIcon, SetWorldDisplayStatus,
        },
    },
};

pub fn routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    Arc<Engine>: FromRef<S>,
    Arc<dyn Access>: FromRef<S>,
{
    Router::new()
        .endpoint::<ListInstances, _, _>(list_instances)
        .endpoint::<GetInstances, _, _>(get_instances)
        .endpoint::<GetInstance, _, _>(get_instance)
        .endpoint::<RemoveInstance, _, _>(remove_instance)
        .endpoint::<EditInstance, _, _>(edit_instance)
        .endpoint::<KillInstance, _, _>(kill_instance)
        .endpoint::<GetOptimalJava, _, _>(get_optimal_java)
        .endpoint::<ListInstanceProjects, _, _>(list_instance_projects)
        .endpoint::<ListInstalledProjectIds, _, _>(list_installed_project_ids)
        .endpoint::<CheckProjectInstalled, _, _>(check_project_installed)
        .endpoint::<ListInstallCandidates, _, _>(list_install_candidates)
        .endpoint::<ListContentItems, _, _>(list_content_items)
        .endpoint::<RefreshContentUpdates, _, _>(refresh_content_updates)
        .endpoint::<SyncContentFiles, _, _>(sync_content_files)
        .endpoint::<ListDependencyContentItems, _, _>(
            list_dependency_content_items,
        )
        .endpoint::<GetLinkedModpack, _, _>(get_linked_modpack)
        .endpoint::<ListLinkedModpackContent, _, _>(list_linked_modpack_content)
        .endpoint::<AddProjectFromVersion, _, _>(add_project_from_version)
        .endpoint::<InstallProjectWithDependencies, _, _>(
            install_project_with_dependencies,
        )
        .endpoint::<UpdateProject, _, _>(update_project)
        .endpoint::<SwitchProjectVersion, _, _>(switch_project_version)
        .endpoint::<ToggleProject, _, _>(toggle_project)
        .endpoint::<SetProjectLocked, _, _>(set_project_locked)
        .endpoint::<RemoveProject, _, _>(remove_project)
        .endpoint::<InstallModpack, _, _>(install_modpack)
        .endpoint::<UpdateModpackVersion, _, _>(update_modpack_version)
        .endpoint::<UnlinkModpack, _, _>(unlink_modpack)
        .endpoint::<RepairModpack, _, _>(repair_modpack)
        .endpoint::<SetSide, _, _>(set_side)
        .endpoint::<SetSidesUnlocked, _, _>(set_sides_unlocked)
        .endpoint::<CompleteSideReview, _, _>(complete_side_review)
        .endpoint::<RepairContent, _, _>(repair_content)
        .endpoint::<EditedFiles, _, _>(edited_files)
        .endpoint::<DecideEdited, _, _>(decide_edited)
        .endpoint::<PlaceClientOnly, _, _>(place_client_only)
        .endpoint::<ListPackExportCandidates, _, _>(list_pack_export_candidates)
        .endpoint::<ListInstanceGroups, _, _>(list_instance_groups)
        .endpoint::<CreateInstanceGroup, _, _>(create_instance_group)
        .endpoint::<RenameInstanceGroup, _, _>(rename_instance_group)
        .endpoint::<DeleteInstanceGroup, _, _>(delete_instance_group)
        .endpoint::<SetInstanceGroupOrder, _, _>(set_instance_group_order)
        .endpoint::<SetInstanceGroupMemberships, _, _>(
            set_instance_group_memberships,
        )
        .endpoint::<ListLoadingBars, _, _>(list_loading_bars)
        .endpoint::<ListRecentWorlds, _, _>(list_recent_worlds)
        .endpoint::<GetServerStatus, _, _>(get_server_status)
        .endpoint::<ListInstanceWorlds, _, _>(list_instance_worlds)
        .endpoint::<SetWorldDisplayStatus, _, _>(set_world_display_status)
        .endpoint::<GetInstanceProtocolVersion, _, _>(
            get_instance_protocol_version,
        )
        .endpoint::<GetSingleplayerWorld, _, _>(get_singleplayer_world)
        .endpoint::<DeleteWorld, _, _>(delete_world)
        .endpoint::<RenameWorld, _, _>(rename_world)
        .endpoint::<ResetWorldIcon, _, _>(reset_world_icon)
        .endpoint::<BackupWorld, _, _>(backup_world)
        .endpoint::<AddInstanceServer, _, _>(add_instance_server)
        .endpoint::<EnsureManagedInstanceServer, _, _>(
            ensure_managed_instance_server,
        )
        .endpoint::<DesyncInstanceServer, _, _>(desync_instance_server)
        .endpoint::<EditInstanceServer, _, _>(edit_instance_server)
        .endpoint::<RemoveInstanceServer, _, _>(remove_instance_server)
        .endpoint::<GetStoreUsage, _, _>(get_store_usage)
        .endpoint::<CleanStore, _, _>(clean_store)
        .endpoint::<SetStoreCacheLimit, _, _>(set_store_cache_limit)
        .endpoint::<GetCategoryTags, _, _>(get_category_tags)
        .endpoint::<GetLoaderTags, _, _>(get_loader_tags)
        .endpoint::<GetGameVersionTags, _, _>(get_game_version_tags)
        .endpoint::<GetLoaderManifest, _, _>(get_loader_manifest)
        .endpoint::<GetCachedProject, _, _>(get_cached_project)
        .endpoint::<GetCachedProjects, _, _>(get_cached_projects)
        .endpoint::<GetCachedProjectVersions, _, _>(get_cached_project_versions)
        .endpoint::<GetCachedProjectV3, _, _>(get_cached_project_v3)
        .endpoint::<GetCachedProjectsV3, _, _>(get_cached_projects_v3)
        .endpoint::<GetCachedVersion, _, _>(get_cached_version)
        .endpoint::<GetCachedVersions, _, _>(get_cached_versions)
        .endpoint::<GetCachedUser, _, _>(get_cached_user)
        .endpoint::<GetCachedUsers, _, _>(get_cached_users)
        .endpoint::<GetCachedTeam, _, _>(get_cached_team)
        .endpoint::<GetCachedOrganization, _, _>(get_cached_organization)
        .endpoint::<GetCachedSearchResults, _, _>(get_cached_search_results)
        .endpoint::<GetCachedSearchResultsV3, _, _>(
            get_cached_search_results_v3,
        )
        .endpoint::<PurgeCache, _, _>(purge_cache)
        .endpoint::<ListInstanceLogs, _, _>(list_instance_logs)
        .endpoint::<GetInstanceLogOutput, _, _>(get_instance_log_output)
        .endpoint::<DeleteInstanceLog, _, _>(delete_instance_log)
        .endpoint::<GetLiveLog, _, _>(get_live_log)
        .endpoint::<ClearLiveLog, _, _>(clear_live_log)
        .endpoint::<ListProcesses, _, _>(list_processes)
        .endpoint::<ListInstanceProcesses, _, _>(list_instance_processes)
        .endpoint::<KillProcess, _, _>(kill_process)
        .endpoint::<GetJavaVersions, _, _>(get_java_versions)
        .endpoint::<SetJavaVersion, _, _>(set_java_version)
        .endpoint::<FindJavaInstalls, _, _>(find_java_installs)
        .endpoint::<InstallJava, _, _>(install_java)
        .endpoint::<GetSystemMemory, _, _>(get_system_memory)
        .endpoint::<ListInstanceFiles, _, _>(list_instance_files)
        .endpoint::<ReadInstanceFile, _, _>(read_instance_file)
        // The host sets the body cap; an upload is often past axum's 2 MB default.
        .endpoint::<WriteInstanceFile, _, _>(write_instance_file)
        .endpoint::<CreateInstanceDirectory, _, _>(create_instance_directory)
        .endpoint::<MoveInstanceFile, _, _>(move_instance_file)
        .endpoint::<DeleteInstanceFile, _, _>(delete_instance_file)
        .endpoint::<ExtractInstanceZip, _, _>(extract_instance_zip)
        .endpoint::<StatInstanceFile, _, _>(stat_instance_file)
        .endpoint::<ZipInstanceFiles, _, _>(zip_instance_files)
        .endpoint::<OpenInstanceUpload, _, _>(open_instance_upload)
        .endpoint::<GetInstanceUpload, _, _>(get_instance_upload)
        .endpoint::<AddInstanceUploadFile, _, _>(add_instance_upload_file)
        .endpoint::<FinalizeInstanceUpload, _, _>(finalize_instance_upload)
        .endpoint::<CancelInstanceUpload, _, _>(cancel_instance_upload)
        .endpoint::<GetInstanceIcon, _, _>(get_instance_icon)
        .endpoint::<SetInstanceIcon, _, _>(set_instance_icon)
        .endpoint::<SetEditedInstanceIcon, _, _>(set_edited_instance_icon)
        .endpoint::<RemoveInstanceIcon, _, _>(remove_instance_icon)
        .endpoint::<InstallInstance, _, _>(install_instance)
        .endpoint::<DuplicateInstance, _, _>(duplicate_instance)
        .endpoint::<UpdateInstanceContent, _, _>(update_instance_content)
        .endpoint::<GetSharedInstanceUpdatePreview, _, _>(
            get_shared_instance_update_preview,
        )
        .endpoint::<UpdateSharedInstance, _, _>(update_shared_instance)
        .endpoint::<GetSharedInstanceHistory, _, _>(get_shared_instance_history)
        .endpoint::<RestoreSharedInstanceVersion, _, _>(
            restore_shared_instance_version,
        )
        .endpoint::<DeleteSharedInstanceVersion, _, _>(
            delete_shared_instance_version,
        )
        .endpoint::<GetSharedInstanceInstallPreview, _, _>(
            get_shared_instance_install_preview,
        )
        .endpoint::<InstallSharedInstance, _, _>(install_shared_instance)
        .endpoint::<AcceptSharedInstanceInvite, _, _>(
            accept_shared_instance_invite,
        )
        .endpoint::<AcceptPendingSharedInstanceInvite, _, _>(
            accept_pending_shared_instance_invite,
        )
        .endpoint::<GetSharedInstanceInvitePreview, _, _>(
            get_shared_instance_invite_preview,
        )
        .endpoint::<GetSharedHistorySettings, _, _>(get_shared_history_settings)
        .endpoint::<GetSharedInstanceUsers, _, _>(get_shared_instance_users)
        .endpoint::<InviteSharedInstanceUsers, _, _>(
            invite_shared_instance_users,
        )
        .endpoint::<RemoveSharedInstanceUsers, _, _>(
            remove_shared_instance_users,
        )
        .endpoint::<ListSharedInstanceInvites, _, _>(
            list_shared_instance_invites,
        )
        .endpoint::<CreateSharedInstanceInviteLink, _, _>(
            create_shared_instance_invite_link,
        )
        .endpoint::<RevokeSharedInstanceInvite, _, _>(
            revoke_shared_instance_invite,
        )
        .endpoint::<GetSharedInstancePublishPreview, _, _>(
            get_shared_instance_publish_preview,
        )
        .endpoint::<GetSharedInstancePublishSupportDetails, _, _>(
            get_shared_instance_publish_support_details,
        )
        .endpoint::<EnsureSharedInstance, _, _>(ensure_shared_instance)
        .endpoint::<UnpublishSharedInstance, _, _>(unpublish_shared_instance)
        .endpoint::<UnlinkSharedInstance, _, _>(unlink_shared_instance)
        .endpoint::<DismissSharedInstanceRestoredNotice, _, _>(
            dismiss_shared_instance_restored_notice,
        )
        .endpoint::<UnpublishRemoteSharedInstance, _, _>(
            unpublish_remote_shared_instance,
        )
        .endpoint::<LeaveRemoteSharedInstance, _, _>(
            leave_remote_shared_instance,
        )
        .endpoint::<SetSharedHistoryKept, _, _>(set_shared_history_kept)
        .endpoint::<ListInstallJobs, _, _>(list_install_jobs)
        .endpoint::<GetInstallJob, _, _>(get_install_job)
        .endpoint::<RetryInstallJob, _, _>(retry_install_job)
        .endpoint::<PauseInstallJob, _, _>(pause_install_job)
        .endpoint::<ResumeInstallJob, _, _>(resume_install_job)
        .endpoint::<CancelInstallJob, _, _>(cancel_install_job)
        .endpoint::<DismissInstallJob, _, _>(dismiss_install_job)
        .endpoint::<GetInstallJobSupportDetails, _, _>(
            get_install_job_support_details,
        )
}

/// An engine error as the route's answer, coded as the Tauri commands code it.
pub fn api_error(error: crate::Error) -> ApiError {
    tracing::warn!("{error:?}");
    let code = crate::error_code(&error);
    let code = ErrorCode::deserialize(serde::de::value::StrDeserializer::<
        serde::de::value::Error,
    >::new(code))
    .unwrap_or(ErrorCode::Unknown);
    ApiError::new(code, format!("{error:#}"))
}
