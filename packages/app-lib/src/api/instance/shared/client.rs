//! The App's side of the backend's share routes. The calls themselves are
//! `sharing_client::backend::Backend`, the client Core uses too; this file adds what is the App's
//! own: the signed-in session as the token, the API semaphore, and the App's errors.

use super::install::*;
use super::*;
use crate::ErrorKind;
use sharing_client::backend::{Backend, BackendError, ShareIcon, Unavailable};
pub(super) use sharing_client::versions::{ManifestFile, ShareVersion};

#[derive(Debug)]
pub(super) enum SharedInstanceRemoteResponse<T> {
    Available(T),
    Unavailable(SharedInstanceUnavailableReason),
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct RemoteInstanceResponse {
    pub(super) name: String,
    pub(super) icon: Option<String>,
    #[serde(default)]
    pub(super) linked_server:
        Option<crate::install::model::SharedInstanceLinkedServer>,
    #[serde(default)]
    pub(super) quarantine: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ExternalFileCandidate {
    pub(super) file_name: String,
    pub(super) file_type: String,
    pub(super) source: ExternalFileSource,
}

#[derive(Clone, Debug)]
pub(super) enum ExternalFileSource {
    InstanceFile(String),
    SharedFile(std::path::PathBuf),
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct InstanceInviteResponse {
    pub(super) id: String,
    pub(super) expiration: DateTime<Utc>,
    pub(super) max_uses: i32,
    pub(super) uses: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct InstanceInviteInfoResponse {
    pub(super) instance_id: String,
    pub(super) instance_name: String,
    #[serde(default)]
    pub(super) instance_icon: Option<String>,
    #[serde(default)]
    pub(super) inviter: Option<SharedInstanceInviteCreator>,
    #[serde(default)]
    pub(super) instance_icon_config: Option<IconConfig>,
    #[serde(default)]
    pub(super) managers: Vec<InstanceInviteManagerResponse>,
    #[serde(default)]
    pub(super) version: Option<ShareVersion>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum InstanceInviteManagerResponse {
    User { id: String },
    Server { name: String, icon: Option<String> },
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct RemoteSharedInstanceUsers {
    #[serde(default)]
    pub(super) users: Vec<SharedInstanceUser>,
    #[serde(default)]
    pub(super) tokens: i32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum RemoteSharedInstanceUsersResponse {
    Current(RemoteSharedInstanceUsers),
    Legacy(Vec<String>),
}

impl RemoteSharedInstanceUsersResponse {
    pub(super) fn into_shared_users(self) -> SharedInstanceUsers {
        match self {
            Self::Current(response) => {
                SharedInstanceUsers::from_users(response.users, response.tokens)
            }
            Self::Legacy(user_ids) => {
                SharedInstanceUsers::from_user_ids(user_ids)
            }
        }
    }
}

/// The loader an instance installs a version with. Servers without a client loader (vanilla,
/// Paper, Purpur) are installed as Fabric so players can add client-side mods on top.
pub(super) fn instance_loader(version: &ShareVersion) -> (ModLoader, String) {
    match ModLoader::from_string(&version.content.loader) {
        ModLoader::Vanilla => (ModLoader::Fabric, String::new()),
        loader => (loader, version.content.loader_version.clone()),
    }
}

/// Refuses a version whose removed files the App would not apply.
pub(super) fn check_removed_files(version: &ShareVersion) -> crate::Result<()> {
    version
        .check_removed_files()
        .map_err(|error| ErrorKind::InputError(error.to_string()).into())
}

/// The share routes as the signed-in account. Uploads and pushes take `long` to have no
/// overall timeout.
fn backend_with(long: bool) -> crate::Result<Backend> {
    let session = shared_clients_session()
        .context("Amberite shared clients service is not configured")?;
    let token = session.access_token.ok_or(ErrorKind::NotLoggedIn)?;
    let https = session.base_url.starts_with("https://");
    let http = match (long, https) {
        (false, true) => &REQWEST_CLIENT,
        (false, false) => &INSECURE_REQWEST_CLIENT,
        (true, true) => &NO_TIMEOUT_REQWEST_CLIENT,
        (true, false) => &INSECURE_NO_TIMEOUT_REQWEST_CLIENT,
    };
    Ok(Backend::new((*http).clone(), session.base_url, token))
}

/// The share routes as the signed-in account.
pub(crate) fn share_backend() -> crate::Result<Backend> {
    backend_with(false)
}

/// The share routes as the signed-in account, without an overall timeout, for a push.
pub(super) fn push_backend() -> crate::Result<Backend> {
    backend_with(true)
}

/// The App's error for a failed call. 5xx and unreachable backends are
/// `SharedInstancesApiError`; refusals keep the backend's status.
pub(super) fn backend_error(error: BackendError) -> crate::Error {
    match error {
        BackendError::Http(error) => anyhow::Error::new(error)
            .context(ErrorKind::SharedInstancesApiError)
            .into(),
        error
            if error
                .status()
                .is_some_and(|status| status.is_server_error()) =>
        {
            anyhow::Error::new(error)
                .context(ErrorKind::SharedInstancesApiError)
                .into()
        }
        error @ BackendError::Decode { .. } => anyhow::Error::new(error)
            .context(ErrorKind::SharedInstancesApiError)
            .into(),
        error => anyhow::Error::new(error).into(),
    }
}

pub(super) fn unavailable_reason(
    reason: Unavailable,
) -> SharedInstanceUnavailableReason {
    match reason {
        Unavailable::Deleted => SharedInstanceUnavailableReason::Deleted,
        Unavailable::AccessRevoked => {
            SharedInstanceUnavailableReason::AccessRevoked
        }
    }
}

/// Runs one call while holding the API semaphore.
async fn call<T>(
    state: &State,
    request: impl Future<Output = Result<T, BackendError>>,
) -> crate::Result<T> {
    let _permit = state.api_semaphore.0.acquire().await?;
    request.await.map_err(backend_error)
}

/// Runs one call on a share, telling an unavailable share apart. A revoked token without a
/// session means the account signed out.
async fn call_share<T>(
    state: &State,
    request: impl Future<Output = Result<T, BackendError>>,
) -> crate::Result<SharedInstanceRemoteResponse<T>> {
    let _permit = state.api_semaphore.0.acquire().await?;
    match request.await {
        Ok(value) => Ok(SharedInstanceRemoteResponse::Available(value)),
        Err(BackendError::Unavailable { reason, .. }) => {
            if reason == Unavailable::AccessRevoked
                && !active_amberite_session_is_configured(state).await?
            {
                bail!(ErrorKind::NotLoggedIn);
            }
            Ok(SharedInstanceRemoteResponse::Unavailable(
                unavailable_reason(reason),
            ))
        }
        Err(error) => Err(backend_error(error)),
    }
}

pub(super) async fn create_remote_instance(
    name: String,
    state: &State,
) -> crate::Result<String> {
    let backend = share_backend()?;
    call(state, backend.create_share(&name)).await
}

pub(super) async fn delete_remote_instance(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    let backend = share_backend()?;
    call(state, backend.delete_share(shared_instance_id)).await
}

pub(super) async fn update_remote_instance(
    shared_instance_id: &str,
    name: String,
    state: &State,
) -> crate::Result<()> {
    let backend = share_backend()?;
    let _permit = state.api_semaphore.0.acquire().await?;
    match backend.rename_share(shared_instance_id, &name).await {
        Err(BackendError::Status {
            status: StatusCode::METHOD_NOT_ALLOWED,
            ..
        }) => {
            tracing::warn!(
                "Shared instances API does not support remote instance updates; skipping name sync"
            );
            Ok(())
        }
        result => result.map_err(backend_error),
    }
}

pub(super) async fn get_remote_instance_access(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<SharedInstanceRemoteResponse<RemoteInstanceResponse>> {
    let backend = share_backend()?;
    Ok(
        match call_share(
            state,
            backend.share::<RemoteInstanceResponse>(shared_instance_id),
        )
        .await?
        {
            SharedInstanceRemoteResponse::Available(instance)
                if instance.quarantine =>
            {
                SharedInstanceRemoteResponse::Unavailable(
                    SharedInstanceUnavailableReason::Quarantined,
                )
            }
            response => response,
        },
    )
}

pub(super) async fn update_remote_instance_icon(
    shared_instance_id: &str,
    icon_path: Option<&str>,
    icon_config: Option<&IconConfig>,
    state: &State,
) -> crate::Result<()> {
    // An icon editor icon goes up as its settings alone; members draw it themselves.
    let icon = match (icon_config, icon_path) {
        (Some(config), _) => Some(ShareIcon::Drawn(config)),
        (None, Some(icon_path)) => {
            Some(ShareIcon::Image(crate::util::io::read(icon_path).await?))
        }
        (None, None) => None,
    };
    let backend = backend_with(matches!(icon, Some(ShareIcon::Image(_))))?;
    call(state, backend.set_share_icon(shared_instance_id, icon)).await
}

pub(super) async fn get_remote_users(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<SharedInstanceUsers> {
    let backend = share_backend()?;
    let users = call(
        state,
        backend.users::<RemoteSharedInstanceUsersResponse>(shared_instance_id),
    )
    .await?;
    Ok(users.into_shared_users())
}

/// The icon of the server a player came through, or `None` when it has none. With `etag`, a
/// 304 answer when it is unchanged.
pub(super) async fn get_remote_server_icon(
    instance_id: &str,
    etag: Option<&str>,
    state: &State,
) -> crate::Result<Option<reqwest::Response>> {
    let backend =
        share_backend()?.with_timeout(std::time::Duration::from_secs(15));
    call(state, backend.server_icon(instance_id, etag)).await
}

pub(super) async fn get_latest_remote_version(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<ShareVersion> {
    match get_latest_remote_version_optional_unavailable(
        shared_instance_id,
        state,
    )
    .await?
    {
        SharedInstanceRemoteResponse::Available(version) => Ok(version),
        SharedInstanceRemoteResponse::Unavailable(reason) => {
            Err(shared_instance_unavailable_error(reason))
        }
    }
}

pub(super) async fn get_latest_remote_version_optional_unavailable(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<SharedInstanceRemoteResponse<ShareVersion>> {
    let backend = share_backend()?;
    call_share(state, backend.version(shared_instance_id, None)).await
}

pub(super) async fn add_remote_users(
    shared_instance_id: &str,
    user_ids: Vec<String>,
    state: &State,
) -> crate::Result<()> {
    let backend = share_backend()?;
    let failed =
        call(state, backend.add_users(shared_instance_id, &user_ids)).await?;
    if !failed.is_empty() {
        bail!(ErrorKind::InputError(format!(
            "{} of the invited accounts do not allow sharing invitations in their privacy settings",
            failed.len()
        )));
    }
    Ok(())
}

pub(super) async fn remove_remote_users(
    shared_instance_id: &str,
    user_ids: Vec<String>,
    state: &State,
) -> crate::Result<()> {
    let backend = share_backend()?;
    call(state, backend.remove_users(shared_instance_id, &user_ids)).await
}

pub(super) async fn accept_pending_remote_invite(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<bool> {
    let backend = share_backend()?;
    call(state, backend.accept_pending_invite(shared_instance_id)).await
}

pub(super) async fn accept_shared_instance_invite(
    shared_instance_id: &str,
    invite_id: &str,
    state: &State,
) -> crate::Result<()> {
    let backend = share_backend()?;
    call(state, backend.accept_invite(shared_instance_id, invite_id)).await
}

pub(super) async fn create_remote_invite(
    shared_instance_id: &str,
    max_age_seconds: i32,
    max_uses: i32,
    state: &State,
) -> crate::Result<String> {
    let backend = share_backend()?;
    call(
        state,
        backend.create_invite(shared_instance_id, max_age_seconds, max_uses),
    )
    .await
}

pub(super) async fn delete_remote_invite(
    shared_instance_id: &str,
    invite_id: &str,
    state: &State,
) -> crate::Result<()> {
    let backend = share_backend()?;
    call(state, backend.delete_invite(shared_instance_id, invite_id)).await
}

pub(super) async fn get_remote_invites(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<Vec<InstanceInviteResponse>> {
    let backend = share_backend()?;
    call(state, backend.invites(shared_instance_id)).await
}

/// An invite's preview, which anyone may read, signed in or not.
pub(super) async fn get_shared_instance_invite_info(
    invite_id: &str,
    state: &State,
) -> crate::Result<InstanceInviteInfoResponse> {
    let session = shared_clients_session()
        .context("Amberite shared clients service is not configured")?;
    let http = if session.base_url.starts_with("https://") {
        &REQWEST_CLIENT
    } else {
        &INSECURE_REQWEST_CLIENT
    };
    let backend = Backend::anonymous((*http).clone(), session.base_url);
    call(state, backend.invite(invite_id)).await
}

pub(super) async fn decline_pending_remote_invite(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    let backend = share_backend()?;
    call(state, backend.decline_pending_invite(shared_instance_id)).await
}

pub(super) async fn active_amberite_session_is_configured(
    _state: &State,
) -> crate::Result<bool> {
    Ok(shared_clients_session()
        .is_some_and(|session| session.access_token.is_some()))
}
