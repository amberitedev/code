//! The App's side of the backend's share routes (`contracts::backend::sharing`), the routes Core
//! calls too: the signed-in session as the token, the API semaphore, the App's errors, and the
//! shapes its commands return.

use super::install::*;
use super::*;
use crate::ErrorKind;
use crate::state::Sessions;
use contracts::backend::sharing::{
    AcceptInvite, AcceptPendingInvite, CreateInvite, CreateSharedInstance,
    DeclinePendingInvite, DeleteInvite, GetInvite, GetLatestVersion,
    GetSharedInstance, InstanceInviteId, InstanceUser, Invite,
    InviteInstanceUsers, InvitePreviewQuery, JoinType, ListInstanceUsers,
    ListInvites, NewInviteLink, NewSharedInstance, RemoveIcon,
    RemoveInstanceUsers, SetGeneratedIcon, SetIcon, SharedInstance,
    SharedInstanceId, SharedInstanceUpdate, UpdateSharedInstance, UserIds,
};
pub(super) use contracts::backend::sharing::{ExternalFile, InstanceVersion};
use contracts::client::{Body, CallError, Reply};
use contracts::{Empty, Endpoint, Json, Raw};
use sharing_client::shares::{OwnedRemote, Unavailable, delete_share};

#[derive(Debug)]
pub(super) enum SharedInstanceRemoteResponse<T> {
    Available(T),
    Unavailable(SharedInstanceUnavailableReason),
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

/// The loader an instance installs a version with. Servers without a client loader (vanilla,
/// Paper, Purpur) are installed as Fabric so players can add client-side mods on top.
pub(super) fn instance_loader(
    version: &InstanceVersion,
) -> (ModLoader, String) {
    let content = &version.manifest.content;
    match ModLoader::from_string(&content.loader) {
        ModLoader::Vanilla => (ModLoader::Fabric, String::new()),
        loader => (loader, content.loader_version.clone()),
    }
}

/// Refuses a version whose removed files the App would not apply.
pub(super) fn check_removed_files(
    version: &InstanceVersion,
) -> crate::Result<()> {
    sharing_client::versions::check_removed_files(version)
        .map_err(|error| ErrorKind::InputError(error.to_string()).into())
}

fn client(base_url: &url::Url, long: bool) -> reqwest::Client {
    let https = base_url.scheme() == "https";
    let http = match (long, https) {
        (false, true) => &REQWEST_CLIENT,
        (false, false) => &INSECURE_REQWEST_CLIENT,
        (true, true) => &NO_TIMEOUT_REQWEST_CLIENT,
        (true, false) => &INSECURE_NO_TIMEOUT_REQWEST_CLIENT,
    };
    (*http).clone()
}

/// The share routes as the signed-in account. Uploads and pushes take `long` to have no
/// overall timeout.
fn backend_with(sessions: &Sessions, long: bool) -> crate::Result<OwnedRemote> {
    if sessions.accounts.is_none() {
        bail!("Amberite shared clients service is not configured");
    }
    let session = sessions.backend().ok_or(ErrorKind::NotLoggedIn)?;
    Ok(OwnedRemote {
        http: client(&session.base_url, long),
        base: session.base_url.as_str().to_owned(),
        bearer: Some(session.bearer),
        timeout: None,
    })
}

/// The share routes as the signed-in account.
pub(crate) fn share_backend(sessions: &Sessions) -> crate::Result<OwnedRemote> {
    backend_with(sessions, false)
}

/// The share routes as the signed-in account, without an overall timeout, for a push.
pub(super) fn push_backend(sessions: &Sessions) -> crate::Result<OwnedRemote> {
    backend_with(sessions, true)
}

/// The App's error for a failed call. 5xx, unreadable answers and unreachable backends are
/// `SharedInstancesApiError`; refusals keep the backend's status.
pub(super) fn backend_error(error: CallError) -> crate::Error {
    if let Some(reason) = Unavailable::of(&error) {
        return anyhow::Error::new(reason).into();
    }
    let server_side = match &error {
        CallError::Api { status, .. } => *status >= 500,
        CallError::Http(_) | CallError::Decode { .. } => true,
        CallError::RangeIgnored { .. } | CallError::Params { .. } => false,
    };
    let error = anyhow::Error::new(error);
    if server_side {
        error.context(ErrorKind::SharedInstancesApiError).into()
    } else {
        error.into()
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

/// One call while holding the API semaphore.
async fn send<E>(
    state: &Engine,
    backend: &OwnedRemote,
    params: &E::Params,
    query: &E::Query,
    body: <E::Body as Body>::Input,
) -> crate::Result<Result<<E::Response as Reply>::Output, CallError>>
where
    E: Endpoint,
    E::Body: Body,
    E::Response: Reply,
{
    let _permit = state.api_semaphore.0.acquire().await?;
    Ok(
        contracts::client::call::<E>(&backend.remote(), params, query, body)
            .await,
    )
}

/// One call on the share `id` as the signed-in account.
async fn call<E>(
    state: &Engine,
    id: &str,
    body: <E::Body as Body>::Input,
) -> crate::Result<<E::Response as Reply>::Output>
where
    E: Endpoint<Params = SharedInstanceId, Query = ()>,
    E::Body: Body,
    E::Response: Reply,
{
    let backend = share_backend(&state.sessions)?;
    send::<E>(state, &backend, &share(id), &(), body)
        .await?
        .map_err(backend_error)
}

/// A call on a share, telling an unavailable share apart. A revoked token without a session
/// means the account signed out.
async fn call_share<T>(
    state: &Engine,
    result: Result<T, CallError>,
) -> crate::Result<SharedInstanceRemoteResponse<T>> {
    match result {
        Ok(value) => Ok(SharedInstanceRemoteResponse::Available(value)),
        Err(error) => match Unavailable::of(&error) {
            Some(reason) => {
                if reason == Unavailable::AccessRevoked
                    && !active_amberite_session_is_configured(state).await?
                {
                    bail!(ErrorKind::NotLoggedIn);
                }
                Ok(SharedInstanceRemoteResponse::Unavailable(
                    unavailable_reason(reason),
                ))
            }
            None => Err(backend_error(error)),
        },
    }
}

fn share(id: &str) -> SharedInstanceId {
    SharedInstanceId { id: id.to_owned() }
}

pub(super) async fn create_remote_instance(
    name: String,
    state: &Engine,
) -> crate::Result<String> {
    let backend = share_backend(&state.sessions)?;
    let body = Json(NewSharedInstance {
        name,
        server_id: None,
    });
    let created = send::<CreateSharedInstance>(state, &backend, &(), &(), body)
        .await?
        .map_err(backend_error)?;
    Ok(created.id)
}

pub(super) async fn delete_remote_instance(
    shared_instance_id: &str,
    state: &Engine,
) -> crate::Result<()> {
    let backend = share_backend(&state.sessions)?;
    let _permit = state.api_semaphore.0.acquire().await?;
    delete_share(&backend.remote(), shared_instance_id)
        .await
        .map_err(backend_error)
}

pub(super) async fn update_remote_instance(
    shared_instance_id: &str,
    name: String,
    state: &Engine,
) -> crate::Result<()> {
    let body = Json(SharedInstanceUpdate { name });
    call::<UpdateSharedInstance>(state, shared_instance_id, body).await
}

pub(super) async fn get_remote_instance_access(
    shared_instance_id: &str,
    state: &Engine,
) -> crate::Result<SharedInstanceRemoteResponse<SharedInstance>> {
    let backend = share_backend(&state.sessions)?;
    let id = share(shared_instance_id);
    let result =
        send::<GetSharedInstance>(state, &backend, &id, &(), Empty).await?;
    Ok(match call_share(state, result).await? {
        SharedInstanceRemoteResponse::Available(instance)
            if instance.quarantine =>
        {
            SharedInstanceRemoteResponse::Unavailable(
                SharedInstanceUnavailableReason::Quarantined,
            )
        }
        response => response,
    })
}

pub(super) async fn update_remote_instance_icon(
    shared_instance_id: &str,
    icon_path: Option<&str>,
    icon_config: Option<&IconConfig>,
    state: &Engine,
) -> crate::Result<()> {
    // An icon editor icon goes up as its settings alone; members draw it themselves.
    match (icon_config, icon_path) {
        (Some(config), _) => {
            call::<SetGeneratedIcon>(
                state,
                shared_instance_id,
                Json(config.clone()),
            )
            .await
        }
        (None, Some(icon_path)) => {
            let bytes = crate::util::io::read(icon_path).await?;
            // The backend serves the image with the type it was sent with.
            let content_type = infer::get(&bytes)
                .map_or("application/octet-stream", |kind| kind.mime_type())
                .to_owned();
            let backend = backend_with(&state.sessions, true)?;
            let image = Raw {
                content_type,
                bytes: bytes.into(),
            };
            send::<SetIcon>(
                state,
                &backend,
                &share(shared_instance_id),
                &(),
                image,
            )
            .await?
            .map_err(backend_error)
        }
        (None, None) => {
            call::<RemoveIcon>(state, shared_instance_id, Empty).await
        }
    }
}

pub(super) async fn get_remote_users(
    shared_instance_id: &str,
    state: &Engine,
) -> crate::Result<SharedInstanceUsers> {
    let users =
        call::<ListInstanceUsers>(state, shared_instance_id, Empty).await?;
    Ok(SharedInstanceUsers::from_users(
        users.users.into_iter().map(shared_instance_user).collect(),
        users.tokens as i32,
    ))
}

fn shared_instance_user(user: InstanceUser) -> SharedInstanceUser {
    SharedInstanceUser {
        id: user.id,
        joined_at: user.joined_at.as_deref().and_then(timestamp),
        join_type: match user.join_type {
            JoinType::Owner => SharedInstanceJoinType::Owner,
            JoinType::Invite => SharedInstanceJoinType::Invite,
            JoinType::Link => SharedInstanceJoinType::Link,
        },
        last_played: user.last_played.as_deref().and_then(timestamp),
    }
}

fn timestamp(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

pub(super) async fn get_latest_remote_version(
    shared_instance_id: &str,
    state: &Engine,
) -> crate::Result<InstanceVersion> {
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
    state: &Engine,
) -> crate::Result<SharedInstanceRemoteResponse<InstanceVersion>> {
    let backend = share_backend(&state.sessions)?;
    let id = share(shared_instance_id);
    let result = send::<GetLatestVersion>(
        state,
        &backend,
        &id,
        &Default::default(),
        Empty,
    )
    .await?;
    call_share(state, result).await
}

pub(super) async fn add_remote_users(
    shared_instance_id: &str,
    user_ids: Vec<String>,
    state: &Engine,
) -> crate::Result<()> {
    let body = Json(UserIds { user_ids });
    let invited =
        call::<InviteInstanceUsers>(state, shared_instance_id, body).await?;
    if !invited.failed.is_empty() {
        bail!(ErrorKind::InputError(format!(
            "{} of the invited accounts do not allow sharing invitations in their privacy settings",
            invited.failed.len()
        )));
    }
    Ok(())
}

pub(super) async fn remove_remote_users(
    shared_instance_id: &str,
    user_ids: Vec<String>,
    state: &Engine,
) -> crate::Result<()> {
    let body = Json(UserIds { user_ids });
    call::<RemoveInstanceUsers>(state, shared_instance_id, body).await
}

/// Accepts the caller's pending invite. False when there is none (404).
pub(super) async fn accept_pending_remote_invite(
    shared_instance_id: &str,
    state: &Engine,
) -> crate::Result<bool> {
    let backend = share_backend(&state.sessions)?;
    let id = share(shared_instance_id);
    match send::<AcceptPendingInvite>(state, &backend, &id, &(), Empty).await? {
        Ok(()) => Ok(true),
        Err(error) if Unavailable::of(&error) == Some(Unavailable::Deleted) => {
            Ok(false)
        }
        Err(error) => Err(backend_error(error)),
    }
}

fn invite_id(shared_instance_id: &str, invite_id: &str) -> InstanceInviteId {
    InstanceInviteId {
        id: shared_instance_id.to_owned(),
        invite_id: invite_id.to_owned(),
    }
}

/// Joins the share through an invite link. Joining a share one already has is done.
pub(super) async fn accept_shared_instance_invite(
    shared_instance_id: &str,
    invite: &str,
    state: &Engine,
) -> crate::Result<()> {
    let backend = share_backend(&state.sessions)?;
    let id = invite_id(shared_instance_id, invite);
    send::<AcceptInvite>(state, &backend, &id, &(), Empty)
        .await?
        .map_err(backend_error)
}

pub(super) async fn create_remote_invite(
    shared_instance_id: &str,
    max_age_seconds: i32,
    max_uses: i32,
    state: &Engine,
) -> crate::Result<String> {
    let body = Json(NewInviteLink {
        max_age: Some(max_age_seconds.max(0) as u32),
        max_uses: Some(max_uses.max(0) as u32),
    });
    Ok(call::<CreateInvite>(state, shared_instance_id, body)
        .await?
        .id)
}

pub(super) async fn delete_remote_invite(
    shared_instance_id: &str,
    invite: &str,
    state: &Engine,
) -> crate::Result<()> {
    let backend = share_backend(&state.sessions)?;
    let id = invite_id(shared_instance_id, invite);
    send::<DeleteInvite>(state, &backend, &id, &(), Empty)
        .await?
        .map_err(backend_error)
}

pub(super) async fn get_remote_invites(
    shared_instance_id: &str,
    state: &Engine,
) -> crate::Result<Vec<SharedInstanceInvite>> {
    let links = call::<ListInvites>(state, shared_instance_id, Empty).await?;
    Ok(links
        .into_iter()
        .filter_map(|link| {
            Some(SharedInstanceInvite {
                expiration: timestamp(&link.expiration)?,
                id: link.id,
                max_uses: link.max_uses as i32,
                uses: link.uses as i32,
            })
        })
        .collect())
}

/// An invite's preview, which anyone may read, signed in or not.
pub(super) async fn get_shared_instance_invite_info(
    invite_id: &str,
    state: &Engine,
) -> crate::Result<Invite> {
    let base_url = state
        .sessions
        .accounts
        .as_ref()
        .map(|accounts| &accounts.api)
        .context("Amberite shared clients service is not configured")?;
    let backend = OwnedRemote {
        http: client(base_url, false),
        base: base_url.as_str().to_owned(),
        bearer: None,
        timeout: None,
    };
    let id = contracts::backend::sharing::InviteId {
        id: invite_id.to_owned(),
    };
    let query = InvitePreviewQuery {
        version: Some(true),
    };
    send::<GetInvite>(state, &backend, &id, &query, Empty)
        .await?
        .map_err(backend_error)
}

pub(super) async fn decline_pending_remote_invite(
    shared_instance_id: &str,
    state: &Engine,
) -> crate::Result<()> {
    call::<DeclinePendingInvite>(state, shared_instance_id, Empty).await
}

pub(super) async fn active_amberite_session_is_configured(
    state: &Engine,
) -> crate::Result<bool> {
    Ok(state.sessions.backend().is_some())
}
