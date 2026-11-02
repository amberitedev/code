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
    ContentSourceKind, EditInstance, ModLoader, ProjectType,
    SharedInstanceRole, State,
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
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, RwLock};

#[derive(Clone)]
pub(crate) struct SharedClientsSession {
    pub base_url: String,
    pub access_token: Option<String>,
    pub user_id: Option<String>,
}

static SHARED_CLIENTS_SESSION: LazyLock<RwLock<Option<SharedClientsSession>>> =
    LazyLock::new(|| RwLock::new(None));

pub fn set_shared_clients_session(
    base_url: String,
    access_token: Option<String>,
    user_id: Option<String>,
) {
    let session = Some(SharedClientsSession {
        base_url: base_url.trim_end_matches('/').to_string(),
        access_token: access_token.filter(|token| !token.trim().is_empty()),
        user_id: user_id.filter(|id| !id.trim().is_empty()),
    });
    *SHARED_CLIENTS_SESSION
        .write()
        .expect("shared clients session lock poisoned") = session;
}

pub(crate) fn shared_clients_session() -> Option<SharedClientsSession> {
    SHARED_CLIENTS_SESSION
        .read()
        .expect("shared clients session lock poisoned")
        .clone()
}

pub(crate) use bedrock::limits::MAX_SHARE_BYTES as MAX_SHARED_FILES_TOTAL_SIZE;
use sharing_client::rules::ShareScope;
pub(crate) use sharing_client::rules::{
    CONFIG_EXTENSIONS as CONFIG_FILE_EXTENSIONS, CONFIG_FILE_TYPE,
    SCAN_HEAD_SIZE as FILE_SCAN_HEAD_SIZE,
};
use sharing_client::versions::RemovedFile;
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
mod icons;
mod install;
mod invites;
mod publish;
mod report;
mod server;
mod types;

pub(crate) use self::client::share_backend;
pub(crate) use self::content::{set_applied_files, shared_modpack_files};
pub(crate) use self::history::last_shared_push;
pub use self::icons::cache_shared_instance_server_icon;
pub(crate) use self::install::{
    check_shared_instance_availability_before_launch,
    shared_instance_update_data,
};
pub(crate) use self::publish::sync_shared_instance_icon;

pub use self::history::{
    delete_shared_instance_version, get_shared_history_settings,
    get_shared_instance_history, restore_shared_instance_version,
    set_shared_history_kept,
};
pub use self::install::{
    accept_shared_instance_invite_for_install,
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
pub use self::publish::{
    get_shared_instance_publish_preview, publish_shared_instance,
    unlink_shared_instance, unpublish_shared_instance,
};
pub use self::report::get_shared_instance_publish_support_details;
pub use self::server::ensure_shared_instance;
pub use self::types::{
    SharedHistorySettings, SharedInstanceExternalFilePreview,
    SharedInstanceHistoryEntry, SharedInstanceInstallPreview,
    SharedInstanceInvite, SharedInstanceInviteCreator,
    SharedInstanceInviteInstallPreview, SharedInstanceInviteLink,
    SharedInstanceJoinType, SharedInstancePublishFile,
    SharedInstancePublishPreview, SharedInstanceUpdateDiff,
    SharedInstanceUpdateDiffType, SharedInstanceUpdatePreview,
    SharedInstanceUser, SharedInstanceUsers, SharedInstanceVersionDetails,
};

pub async fn can_active_user_use_shared_instances() -> crate::Result<bool> {
    Ok(shared_clients_session()
        .is_some_and(|session| session.access_token.is_some()))
}
