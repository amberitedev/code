use crate::SharedInstanceUnavailableReason;
use crate::event::InstancePayloadType;
use crate::event::emit::emit_instance;
use crate::install::{
    InstallJobSnapshot, SharedInstanceExternalFileData,
    SharedInstanceInstallData,
};
use crate::state::instances::{InstanceLink, SharedInstanceAttachment};
use crate::state::{
    AppliedContentSetPatch, CacheBehaviour, CachedEntry, ContentSetSyncStatus,
    ContentSourceKind, EditInstance, Engine, IconConfig, ModLoader,
    ProjectType, SharedInstanceRole,
};
use crate::util::fetch::{
    INSECURE_NO_TIMEOUT_REQWEST_CLIENT, INSECURE_REQWEST_CLIENT,
    NO_TIMEOUT_REQWEST_CLIENT, REQWEST_CLIENT,
};
use anyhow::{Context, bail};
use chrono::{DateTime, Utc};
use modrinth_content_management::{
    CommonExternalFilePolicy, ContentSetConfiguration, ExternalFileKey,
    LoaderReference,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::collections::{HashMap, HashSet};

use contracts::backend::sharing::RemovedFile;
pub(crate) use contracts::rules::SHAREABLE_CONFIG_EXTENSIONS as CONFIG_FILE_EXTENSIONS;
pub(crate) use sharing_client::rules::CONFIG_FILE_TYPE;
use sharing_client::rules::ShareScope;
pub(crate) const CONFIG_SYNC_ENABLED: bool = true;
pub(crate) const CONFIG_DIRECTORY: &str = "config";

/// Where an instance's shared files come from: any path its `.mrpack` export includes.
pub(crate) const INSTANCE_SCOPE: ShareScope = ShareScope::Instance {
    exportable: super::export_mrpack::is_exportable_path,
};

/// Whether members can install a shared file at this instance-relative path.
/// Content folders are shared through the content set instead.
pub(crate) fn is_shareable_file_path(path: &str) -> bool {
    INSTANCE_SCOPE.carries(path)
}

mod client;
mod content;
mod diff;
mod history;
mod install;
mod invites;
mod library;
mod publish;
mod report;
mod server;
mod types;

pub(crate) use self::client::share_backend;
pub(crate) use self::content::{set_applied_files, shared_modpack_files};
pub(crate) use self::install::{
    check_shared_instance_availability_before_launch,
    shared_instance_update_data,
};
pub(crate) use self::publish::sync_shared_instance_icon;

pub use self::history::{
    delete_shared_instance_version, forget_share_history,
    get_shared_history_settings, get_shared_instance_history,
    restore_shared_instance_version, set_shared_history_kept,
};
pub use self::install::{
    accept_shared_instance_invite_for_install,
    dismiss_shared_instance_restored_notice,
    get_shared_instance_install_preview,
    get_shared_instance_invite_install_preview,
    get_shared_instance_update_preview, install_shared_instance,
    update_shared_instance,
};
pub use self::invites::{
    accept_pending_shared_instance_invite, create_shared_instance_invite_link,
    decline_pending_shared_instance_invite, get_shared_instance_invites,
    get_shared_instance_users, invite_shared_instance_users,
    remove_shared_instance_users, revoke_shared_instance_invite,
};
pub use self::library::{
    cache_shared_instance_library, get_cached_shared_instance_library,
};
pub use self::publish::{
    get_shared_instance_publish_preview, leave_remote_shared_instance,
    publish_shared_instance, unlink_shared_instance,
    unpublish_remote_shared_instance, unpublish_shared_instance,
};
pub(crate) use self::report::PublishReports;
pub use self::report::get_shared_instance_publish_support_details;
pub use self::server::{
    ensure_shared_instance, ensure_unpublished_shared_instance,
};
pub use self::types::{
    SharedHistorySettings, SharedInstanceExternalFilePreview,
    SharedInstanceHistoryEntry, SharedInstanceInstallPreview,
    SharedInstanceInvite, SharedInstanceInviteCreator,
    SharedInstanceInviteInstallPreview, SharedInstanceInviteLink,
    SharedInstanceJoinType, SharedInstancePublishFile,
    SharedInstancePublishPreview, SharedInstanceUpdateDiff,
    SharedInstanceUpdateDiffType, SharedInstanceUpdatePreview,
    SharedInstanceUser, SharedInstanceUsers,
};

pub async fn can_active_user_use_shared_instances(
    state: &Engine,
) -> crate::Result<bool> {
    Ok(state.sessions.backend().is_some())
}
