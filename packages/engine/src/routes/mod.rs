//! The engine's HTTP API. Each handler is a `pub fn`, so a host can mount one itself, wrapped in
//! its own hook. A host's state gives the engine and its [`Access`].

mod cache;
mod files;
mod groups;
mod install;
mod instances;
mod jre;
mod loading;
mod logs;
mod metadata;
mod process;
mod store;
mod tags;
#[cfg(test)]
mod tests;
mod worlds;

use std::sync::Arc;

use axum::{
    Router,
    extract::{DefaultBodyLimit, FromRef},
};
use contracts::{ApiError, ErrorCode, server::route};
use serde::Deserialize;

pub use self::{
    cache::*, files::*, groups::*, install::*, instances::*, jre::*,
    loading::*, logs::*, metadata::*, process::*, store::*, tags::*, worlds::*,
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
        files::{
            CreateInstanceDirectory, DeleteInstanceFile, ExtractInstanceZip,
            ListInstanceFiles, MoveInstanceFile, ReadInstanceFile,
            WriteInstanceFile,
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
        instances::{GetInstance, GetInstances, ListInstances},
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
    let (write_path, write) =
        route::<WriteInstanceFile, _, _, _>(write_instance_file);
    [
        route::<ListInstances, _, _, _>(list_instances),
        route::<GetInstances, _, _, _>(get_instances),
        route::<GetInstance, _, _, _>(get_instance),
        route::<ListInstanceGroups, _, _, _>(list_instance_groups),
        route::<CreateInstanceGroup, _, _, _>(create_instance_group),
        route::<RenameInstanceGroup, _, _, _>(rename_instance_group),
        route::<DeleteInstanceGroup, _, _, _>(delete_instance_group),
        route::<SetInstanceGroupOrder, _, _, _>(set_instance_group_order),
        route::<SetInstanceGroupMemberships, _, _, _>(
            set_instance_group_memberships,
        ),
        route::<ListLoadingBars, _, _, _>(list_loading_bars),
        route::<ListRecentWorlds, _, _, _>(list_recent_worlds),
        route::<GetServerStatus, _, _, _>(get_server_status),
        route::<ListInstanceWorlds, _, _, _>(list_instance_worlds),
        route::<SetWorldDisplayStatus, _, _, _>(set_world_display_status),
        route::<GetInstanceProtocolVersion, _, _, _>(
            get_instance_protocol_version,
        ),
        route::<GetSingleplayerWorld, _, _, _>(get_singleplayer_world),
        route::<DeleteWorld, _, _, _>(delete_world),
        route::<RenameWorld, _, _, _>(rename_world),
        route::<ResetWorldIcon, _, _, _>(reset_world_icon),
        route::<BackupWorld, _, _, _>(backup_world),
        route::<AddInstanceServer, _, _, _>(add_instance_server),
        route::<EnsureManagedInstanceServer, _, _, _>(
            ensure_managed_instance_server,
        ),
        route::<DesyncInstanceServer, _, _, _>(desync_instance_server),
        route::<EditInstanceServer, _, _, _>(edit_instance_server),
        route::<RemoveInstanceServer, _, _, _>(remove_instance_server),
        route::<GetStoreUsage, _, _, _>(get_store_usage),
        route::<CleanStore, _, _, _>(clean_store),
        route::<SetStoreCacheLimit, _, _, _>(set_store_cache_limit),
        route::<GetCategoryTags, _, _, _>(get_category_tags),
        route::<GetLoaderTags, _, _, _>(get_loader_tags),
        route::<GetGameVersionTags, _, _, _>(get_game_version_tags),
        route::<GetLoaderManifest, _, _, _>(get_loader_manifest),
        route::<GetCachedProject, _, _, _>(get_cached_project),
        route::<GetCachedProjects, _, _, _>(get_cached_projects),
        route::<GetCachedProjectVersions, _, _, _>(get_cached_project_versions),
        route::<GetCachedProjectV3, _, _, _>(get_cached_project_v3),
        route::<GetCachedProjectsV3, _, _, _>(get_cached_projects_v3),
        route::<GetCachedVersion, _, _, _>(get_cached_version),
        route::<GetCachedVersions, _, _, _>(get_cached_versions),
        route::<GetCachedUser, _, _, _>(get_cached_user),
        route::<GetCachedUsers, _, _, _>(get_cached_users),
        route::<GetCachedTeam, _, _, _>(get_cached_team),
        route::<GetCachedOrganization, _, _, _>(get_cached_organization),
        route::<GetCachedSearchResults, _, _, _>(get_cached_search_results),
        route::<GetCachedSearchResultsV3, _, _, _>(
            get_cached_search_results_v3,
        ),
        route::<PurgeCache, _, _, _>(purge_cache),
        route::<ListInstanceLogs, _, _, _>(list_instance_logs),
        route::<GetInstanceLogOutput, _, _, _>(get_instance_log_output),
        route::<DeleteInstanceLog, _, _, _>(delete_instance_log),
        route::<GetLiveLog, _, _, _>(get_live_log),
        route::<ClearLiveLog, _, _, _>(clear_live_log),
        route::<ListProcesses, _, _, _>(list_processes),
        route::<ListInstanceProcesses, _, _, _>(list_instance_processes),
        route::<KillProcess, _, _, _>(kill_process),
        route::<GetJavaVersions, _, _, _>(get_java_versions),
        route::<SetJavaVersion, _, _, _>(set_java_version),
        route::<FindJavaInstalls, _, _, _>(find_java_installs),
        route::<InstallJava, _, _, _>(install_java),
        route::<GetSystemMemory, _, _, _>(get_system_memory),
        route::<ListInstanceFiles, _, _, _>(list_instance_files),
        route::<ReadInstanceFile, _, _, _>(read_instance_file),
        // An upload is often past axum's 2 MB default.
        (write_path, write.layer(DefaultBodyLimit::disable())),
        route::<CreateInstanceDirectory, _, _, _>(create_instance_directory),
        route::<MoveInstanceFile, _, _, _>(move_instance_file),
        route::<DeleteInstanceFile, _, _, _>(delete_instance_file),
        route::<ExtractInstanceZip, _, _, _>(extract_instance_zip),
        route::<InstallInstance, _, _, _>(install_instance),
        route::<DuplicateInstance, _, _, _>(duplicate_instance),
        route::<UpdateInstanceContent, _, _, _>(update_instance_content),
        route::<GetSharedInstanceUpdatePreview, _, _, _>(
            get_shared_instance_update_preview,
        ),
        route::<UpdateSharedInstance, _, _, _>(update_shared_instance),
        route::<GetSharedInstanceHistory, _, _, _>(get_shared_instance_history),
        route::<RestoreSharedInstanceVersion, _, _, _>(
            restore_shared_instance_version,
        ),
        route::<DeleteSharedInstanceVersion, _, _, _>(
            delete_shared_instance_version,
        ),
        route::<GetSharedInstanceInstallPreview, _, _, _>(
            get_shared_instance_install_preview,
        ),
        route::<InstallSharedInstance, _, _, _>(install_shared_instance),
        route::<AcceptSharedInstanceInvite, _, _, _>(
            accept_shared_instance_invite,
        ),
        route::<AcceptPendingSharedInstanceInvite, _, _, _>(
            accept_pending_shared_instance_invite,
        ),
        route::<GetSharedInstanceInvitePreview, _, _, _>(
            get_shared_instance_invite_preview,
        ),
        route::<GetSharedHistorySettings, _, _, _>(get_shared_history_settings),
        route::<SetSharedHistoryKept, _, _, _>(set_shared_history_kept),
        route::<ListInstallJobs, _, _, _>(list_install_jobs),
        route::<GetInstallJob, _, _, _>(get_install_job),
        route::<RetryInstallJob, _, _, _>(retry_install_job),
        route::<PauseInstallJob, _, _, _>(pause_install_job),
        route::<ResumeInstallJob, _, _, _>(resume_install_job),
        route::<CancelInstallJob, _, _, _>(cancel_install_job),
        route::<DismissInstallJob, _, _, _>(dismiss_install_job),
        route::<GetInstallJobSupportDetails, _, _, _>(
            get_install_job_support_details,
        ),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
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
