use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::instances::InstanceRef;
use crate::{
    install::{ContentUpdateSelection, InstallJobSnapshot},
    instance::{
        SharedHistorySettings, SharedInstanceHistoryEntry,
        SharedInstanceInstallPreview, SharedInstanceInviteInstallPreview,
        SharedInstanceUpdatePreview,
    },
    state::IconConfig,
};

contracts::endpoints! {
    group installs;

    InstallInstance: Post "/instances/{id}/install", Session, InstanceRef, (), Json<InstanceInstall> => Json<InstallJobSnapshot>, client_only;
    DuplicateInstance: Post "/instances/{id}/duplicate", Session, InstanceRef, (), Empty => Json<InstallJobSnapshot>, client_only;
    UpdateInstanceContent: Post "/instances/{id}/content-updates", Session, InstanceRef, (), Json<Vec<ContentUpdateSelection>> => Json<InstallJobSnapshot>, shared, untimed;
    GetSharedInstanceUpdatePreview: Get "/instances/{id}/shared/update-preview", Session, InstanceRef, (), Empty => Json<Option<SharedInstanceUpdatePreview>>, client_only;
    UpdateSharedInstance: Post "/instances/{id}/shared/update", Session, InstanceRef, (), Empty => Json<InstallJobSnapshot>, client_only;
    /// The share's pushed versions, newest first. Empty when the instance joined the share as a
    /// member rather than owning it.
    GetSharedInstanceHistory: Get "/instances/{id}/shared/history", Session, InstanceRef, (), Empty => Json<Vec<SharedInstanceHistoryEntry>>, shared;
    /// Puts the instance back to a version from its history, leaving it to push.
    RestoreSharedInstanceVersion: Post "/instances/{id}/shared/history/{version}/restore", Session, SharedVersionRef, (), Empty => Json<InstallJobSnapshot>, shared, untimed;
    /// The latest version stays.
    DeleteSharedInstanceVersion: Delete "/instances/{id}/shared/history/{version}", Session, SharedVersionRef, (), Empty => Empty, shared;
    GetSharedInstanceInstallPreview: Get "/shared-instances/{id}/install-preview", Session, SharedInstanceRef, SharedInstanceName, Empty => Json<SharedInstanceInstallPreview>, client_only;
    InstallSharedInstance: Post "/shared-instances/{id}/install", Session, SharedInstanceRef, (), Json<SharedInstanceInstall> => Json<InstallJobSnapshot>, client_only;
    /// Joins the share with the invite, unless the caller already has access.
    AcceptSharedInstanceInvite: Post "/shared-instances/{id}/invites/{invite_id}/accept", Session, SharedInstanceInviteRef, (), Empty => Empty, client_only;
    /// Accepts the invite the share's owner sent the caller.
    AcceptPendingSharedInstanceInvite: Post "/shared-instances/{id}/accept", Session, SharedInstanceRef, (), Empty => Empty, client_only;
    GetSharedInstanceInvitePreview: Get "/shared-instance-invites/{id}/install-preview", Session, SharedInstanceRef, (), Empty => Json<SharedInstanceInviteInstallPreview>, client_only;
    GetSharedHistorySettings: Get "/shared-history", Session, (), (), Empty => Json<SharedHistorySettings>, client_only;
    /// Keeps the newest `kept` versions of every share's history (all when `null`) and deletes
    /// the rest.
    SetSharedHistoryKept: Put "/shared-history/kept", Session, (), (), Json<SharedHistoryKept> => Json<SharedHistorySettings>, client_only;
    /// The jobs whose instance the caller may view.
    ListInstallJobs: Get "/install-jobs", Session, (), InstallJobQuery, Empty => Json<Vec<InstallJobSnapshot>>, client_only;
    GetInstallJob: Get "/install-jobs/{id}", Session, InstallJobRef, (), Empty => Json<InstallJobSnapshot>, client_only;
    RetryInstallJob: Post "/install-jobs/{id}/retry", Session, InstallJobRef, (), Empty => Json<InstallJobSnapshot>, client_only;
    PauseInstallJob: Post "/install-jobs/{id}/pause", Session, InstallJobRef, (), Empty => Json<InstallJobSnapshot>, client_only;
    ResumeInstallJob: Post "/install-jobs/{id}/resume", Session, InstallJobRef, (), Empty => Json<InstallJobSnapshot>, client_only;
    CancelInstallJob: Post "/install-jobs/{id}/cancel", Session, InstallJobRef, (), Empty => Json<InstallJobSnapshot>, client_only;
    /// Hides a finished job from the list.
    DismissInstallJob: Delete "/install-jobs/{id}", Session, InstallJobRef, (), Empty => Empty, client_only;
    /// A plain-text report of the job for a support request.
    GetInstallJobSupportDetails: Get "/install-jobs/{id}/support-details", Session, InstallJobRef, (), Empty => Json<String>, client_only;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceInstall {
    pub force: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharedVersionRef {
    pub id: String,
    pub version: i32,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharedInstanceRef {
    pub id: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharedInstanceInviteRef {
    pub id: String,
    pub invite_id: String,
}

/// The name the instance would get.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharedInstanceName {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharedInstanceInstall {
    pub name: String,
    pub manager_id: Option<String>,
    pub server_manager_name: Option<String>,
    pub server_manager_icon_url: Option<String>,
    pub instance_icon_url: Option<String>,
    pub instance_icon_config: Option<IconConfig>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharedHistoryKept {
    pub kept: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstallJobQuery {
    pub include_finished: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstallJobRef {
    pub id: uuid::Uuid,
}
