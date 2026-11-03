//! The App's side of the backend's share routes. The calls themselves are
//! `sharing_client::backend::Backend`, the client Core uses too; this file adds what is the App's
//! own: the signed-in session as the token, the API semaphore, and the App's errors.

use super::install::*;
use super::*;
use crate::ErrorKind;
<<<<<<< HEAD
use sharing_client::backend::{Backend, BackendError, Unavailable};
pub(super) use sharing_client::versions::{ManifestFile, ShareVersion};
=======
use anyhow::{Context, anyhow, bail};

/// Returned by the shared instances backend when the user already owns the maximum number of instances.
const INSTANCE_LIMIT_REACHED_MESSAGE: &str = "Cannot own more than";

#[derive(Clone, Copy, Debug)]
pub(super) enum SharedInstancesRequestAuth {
    ModrinthSession,
    None,
}

impl SharedInstancesRequestAuth {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::ModrinthSession => "modrinth_session",
            Self::None => "none",
        }
    }
}

impl SharedInstanceUnavailableReason {
    pub(super) fn from_status(status: StatusCode) -> Option<Self> {
        match status {
            StatusCode::NOT_FOUND => Some(Self::Deleted),
            StatusCode::UNAUTHORIZED => Some(Self::AccessRevoked),
            _ => None,
        }
    }
}
>>>>>>> modrinth/main

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
<<<<<<< HEAD
    pub(super) inviter: Option<SharedInstanceInviteCreator>,
=======
    pub(super) instance_icon_config: Option<IconConfig>,
>>>>>>> modrinth/main
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
<<<<<<< HEAD
) -> crate::Result<String> {
    let backend = share_backend()?;
    call(state, backend.create_share(&name)).await
=======
) -> crate::Result<CreateInstanceResponse> {
    let operation = "create_instance";
    let method = Method::POST;
    let path = "/instances";
    let response = send_request(
        operation,
        method.clone(),
        path,
        Some(json!({ "name": name })),
        state,
    )
    .await?;

    if response.status() == StatusCode::BAD_REQUEST {
        let status = response.status();
        let request_id = response_request_id(&response);
        let body = response.text().await.unwrap_or_default();
        if body.contains(INSTANCE_LIMIT_REACHED_MESSAGE) {
            bail!(ErrorKind::SharedInstanceLimitReached);
        }
        return Err(shared_instances_status_error(
            operation,
            &method,
            path,
            status,
            request_id.as_deref(),
        ));
    }
    if !response.status().is_success() {
        return shared_instances_request_error(
            operation, method, path, response,
        )
        .await;
    }

    decode_json_response(operation, method, path, response).await
}

pub(super) async fn get_user_blacklist_status(
    user_id: &str,
    state: &State,
) -> crate::Result<BlacklistStatusResponse> {
    request_json(
        "get_user_blacklist_status",
        Method::GET,
        &format!("/blacklist/{user_id}"),
        None,
        state,
    )
    .await
>>>>>>> modrinth/main
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
    let icon = match icon_path {
        Some(icon_path) => Some(crate::util::io::read(icon_path).await?),
        None => None,
    };
<<<<<<< HEAD
    let backend = backend_with(icon.is_some())?;
    call(state, backend.set_share_icon(shared_instance_id, icon)).await
=======

    let bytes = crate::util::io::read(icon_path).await?;
    let method = Method::PUT;
    let (operation, path, response) = if let Some(icon_config) = icon_config {
        let operation = "upload_generated_instance_icon";
        let path = format!("{path}/generated");
        let form = reqwest::multipart::Form::new()
            .part(
                "image",
                reqwest::multipart::Part::bytes(bytes)
                    .file_name("icon.png")
                    .mime_str("image/png")?,
            )
            .part(
                "config",
                reqwest::multipart::Part::text(serde_json::to_string(
                    icon_config,
                )?)
                .mime_str("application/json")?,
            );
        let response = send_multipart_request(
            operation,
            method.clone(),
            &path,
            form,
            state,
        )
        .await?;
        (operation, path, response)
    } else {
        let operation = "upload_instance_icon";
        let response =
            send_bytes_request(operation, method.clone(), &path, bytes, state)
                .await?;
        (operation, path, response)
    };

    if response.status().is_success() {
        return Ok(());
    }

    shared_instances_request_error(operation, method, &path, response).await
>>>>>>> modrinth/main
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
<<<<<<< HEAD
    Ok(shared_clients_session()
        .is_some_and(|session| session.access_token.is_some()))
=======
    let Some(credentials) =
        ModrinthCredentials::get_and_refresh(&state.pool, &state.api_semaphore)
            .await?
    else {
        return Ok(false);
    };

    let _permit = state.api_semaphore.0.acquire().await?;
    let response = INSECURE_REQWEST_CLIENT
        .get(concat!(env!("MODRINTH_API_URL"), "user"))
        .header("Authorization", &credentials.session)
        .send()
        .await?;

    if response.status() == StatusCode::UNAUTHORIZED {
        ModrinthCredentials::remove(&credentials.user_id, &state.pool).await?;
        return Ok(false);
    }

    if response.status().is_success() {
        return Ok(true);
    }

    let status = response.status();
    let request_id = response_request_id(&response);
    tracing::warn!(
        operation = "validate_modrinth_session",
        method = Method::GET.as_str(),
        path = "/user",
        status = status.as_u16(),
        request_id = request_id.as_deref().unwrap_or("none"),
        "Modrinth auth validation request failed"
    );
    bail!("Modrinth auth validation failed with status {status}")
}

pub(super) async fn decode_json_response<T>(
    operation: &'static str,
    method: Method,
    path: &str,
    response: reqwest::Response,
) -> crate::Result<T>
where
    T: DeserializeOwned,
{
    decode_json_response_with_log_path(operation, method, path, response, false)
        .await
}

async fn decode_json_response_with_log_path<T>(
    operation: &'static str,
    method: Method,
    log_path: &str,
    response: reqwest::Response,
    strip_response_url: bool,
) -> crate::Result<T>
where
    T: DeserializeOwned,
{
    let status = response.status();
    let request_id = response_request_id(&response);
    let body = response
        .text()
        .await
        .map_err(|error| {
            if strip_response_url {
                error.without_url()
            } else {
                error
            }
        })
        .context(ErrorKind::SharedInstancesApiError)?;
    serde_json::from_str::<T>(&body)
        .inspect_err(|error| {
            tracing::warn!(
                operation,
                method = method.as_str(),
                path = log_path,
                status = status.as_u16(),
                request_id = request_id.as_deref().unwrap_or("none"),
                error_category = ?error.classify(),
                error_line = error.line(),
                error_column = error.column(),
                "Shared instances API returned an invalid JSON response"
            );
        })
        .context(ErrorKind::SharedInstancesApiError)
}

pub(super) async fn request_empty(
    operation: &'static str,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
    state: &State,
) -> crate::Result<()> {
    request(operation, method, path, body, state).await?;
    Ok(())
}

async fn request_empty_with_log_path(
    operation: &'static str,
    method: Method,
    path: &str,
    log_path: &str,
    body: Option<serde_json::Value>,
    state: &State,
) -> crate::Result<()> {
    let response = send_request_with_auth_and_log_path(
        operation,
        method.clone(),
        path,
        log_path,
        body,
        state,
        SharedInstancesRequestAuth::ModrinthSession,
    )
    .await?;
    if response.status().is_success() {
        return Ok(());
    }

    shared_instances_request_error(operation, method, log_path, response).await
}

pub(super) async fn request(
    operation: &'static str,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
    state: &State,
) -> crate::Result<reqwest::Response> {
    let response =
        send_request(operation, method.clone(), path, body, state).await?;
    if response.status().is_success() {
        return Ok(response);
    }

    shared_instances_request_error(operation, method, path, response).await
}

pub(super) async fn send_request(
    operation: &'static str,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
    state: &State,
) -> crate::Result<reqwest::Response> {
    send_request_with_auth(
        operation,
        method,
        path,
        body,
        state,
        SharedInstancesRequestAuth::ModrinthSession,
    )
    .await
}

pub(super) async fn send_bytes_request(
    operation: &'static str,
    method: Method,
    path: &str,
    body: Vec<u8>,
    state: &State,
) -> crate::Result<reqwest::Response> {
    let base_url = service_base_url();
    let url = service_url(base_url, path);
    send_bytes_request_to_url(operation, method, path, &url, body, None, state)
        .await
}

pub(super) async fn send_multipart_request(
    operation: &'static str,
    method: Method,
    path: &str,
    form: reqwest::multipart::Form,
    state: &State,
) -> crate::Result<reqwest::Response> {
    let base_url = service_base_url();
    let url = service_url(base_url, path);
    let credentials =
        ModrinthCredentials::get_and_refresh(&state.pool, &state.api_semaphore)
            .await?
            .ok_or(crate::ErrorKind::NotLoggedIn)?;
    let _permit = state.api_semaphore.0.acquire().await?;

    tracing::debug!(
        operation,
        method = method.as_str(),
        path,
        user_id = credentials.user_id.as_str(),
        auth = SharedInstancesRequestAuth::ModrinthSession.label(),
        has_body = true,
        "Sending shared instances API request"
    );

    shared_instances_upload_client(base_url)
        .request(method, &url)
        .bearer_auth(credentials.session)
        .multipart(form)
        .send()
        .await
        .map_err(|error| error.without_url())
        .context(ErrorKind::SharedInstancesApiError)
}

pub(super) async fn send_bytes_request_to_url(
    operation: &'static str,
    method: Method,
    path: &str,
    url: &str,
    body: Vec<u8>,
    file_sha512: Option<&str>,
    state: &State,
) -> crate::Result<reqwest::Response> {
    let size = body.len() as u64;
    send_body_request_to_url(
        operation,
        method,
        path,
        url,
        body.into(),
        Some(size),
        file_sha512_headers(file_sha512)?,
        state,
    )
    .await
}

pub(super) fn file_sha512_headers(
    file_sha512: Option<&str>,
) -> crate::Result<reqwest::header::HeaderMap> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(file_sha512) = file_sha512 {
        headers.insert(
            "x-file-sha512",
            reqwest::header::HeaderValue::from_str(file_sha512)
                .context("invalid file SHA-512")?,
        );
    }
    Ok(headers)
}

pub(super) async fn send_body_request_to_url(
    operation: &'static str,
    method: Method,
    path: &str,
    url: &str,
    body: reqwest::Body,
    content_length: Option<u64>,
    headers: reqwest::header::HeaderMap,
    state: &State,
) -> crate::Result<reqwest::Response> {
    let service_origin = url::Url::parse(service_base_url())
        .context("invalid shared instances API base URL")?
        .origin();
    let upload_origin = url::Url::parse(url)
        .context("invalid shared instances upload URL")?
        .origin();
    if service_origin != upload_origin {
        bail!("shared instances upload URL has an unexpected origin");
    }

    let credentials =
        ModrinthCredentials::get_and_refresh(&state.pool, &state.api_semaphore)
            .await?
            .ok_or(crate::ErrorKind::NotLoggedIn)?;
    let _permit = state.api_semaphore.0.acquire().await?;

    tracing::debug!(
        operation,
        method = method.as_str(),
        path,
        url = %url,
        user_id = credentials.user_id.as_str(),
        auth = SharedInstancesRequestAuth::ModrinthSession.label(),
        has_body = true,
        "Sending shared instances API request"
    );

    let mut request = shared_instances_upload_client(url)
        .request(method.clone(), url)
        .bearer_auth(credentials.session)
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .headers(headers)
        .body(body);
    if let Some(content_length) = content_length {
        request =
            request.header(reqwest::header::CONTENT_LENGTH, content_length);
    }

    let response = request
        .send()
        .await
        .map_err(|error| error.without_url())
        .context(ErrorKind::SharedInstancesApiError)?;

    if response.status().is_success() {
        let request_id = response_request_id(&response);
        tracing::debug!(
            operation,
            method = method.as_str(),
            path,
            url = %url,
            status = response.status().as_u16(),
            request_id = request_id.as_deref().unwrap_or("none"),
            "Shared instances API request succeeded"
        );
    }

    Ok(response)
}

pub(super) async fn send_request_with_auth(
    operation: &'static str,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
    state: &State,
    auth: SharedInstancesRequestAuth,
) -> crate::Result<reqwest::Response> {
    send_request_with_auth_and_log_path(
        operation, method, path, path, body, state, auth,
    )
    .await
}

async fn send_request_with_auth_and_log_path(
    operation: &'static str,
    method: Method,
    path: &str,
    log_path: &str,
    body: Option<serde_json::Value>,
    state: &State,
    auth: SharedInstancesRequestAuth,
) -> crate::Result<reqwest::Response> {
    let modrinth_credentials =
        if matches!(auth, SharedInstancesRequestAuth::ModrinthSession) {
            Some(
                ModrinthCredentials::get_and_refresh(
                    &state.pool,
                    &state.api_semaphore,
                )
                .await?
                .ok_or(crate::ErrorKind::NotLoggedIn)?,
            )
        } else {
            None
        };
    let _permit = state.api_semaphore.0.acquire().await?;
    let base_url = service_base_url();
    let url = service_url(base_url, path);
    let log_url = service_url(base_url, log_path);
    let mut request =
        shared_instances_client(base_url).request(method.clone(), &url);
    let mut user_id = None;

    match auth {
        SharedInstancesRequestAuth::ModrinthSession => {
            let credentials = modrinth_credentials
                .expect("Modrinth session credentials were loaded");
            user_id = Some(credentials.user_id);
            request = request.bearer_auth(credentials.session);
        }
        SharedInstancesRequestAuth::None => {}
    }

    tracing::debug!(
        operation,
        method = method.as_str(),
        path = log_path,
        url = %log_url,
        user_id = user_id.as_deref(),
        auth = auth.label(),
        has_body = body.is_some(),
        "Sending shared instances API request"
    );

    if let Some(body) = body {
        request = request.json(&body);
    }

    let response = request
        .send()
        .await
        .map_err(|error| {
            if path != log_path {
                error.without_url()
            } else {
                error
            }
        })
        .context(format!("sending {operation} request"))
        .context(ErrorKind::SharedInstancesApiError)?;
    if response.status().is_success() {
        let request_id = response_request_id(&response);
        tracing::debug!(
            operation,
            method = method.as_str(),
            path = log_path,
            url = %log_url,
            status = response.status().as_u16(),
            request_id = request_id.as_deref().unwrap_or("none"),
            "Shared instances API request succeeded"
        );
    }
    Ok(response)
}

pub(super) async fn shared_instances_request_error<T>(
    operation: &'static str,
    method: Method,
    path: &str,
    response: reqwest::Response,
) -> crate::Result<T> {
    let status = response.status();
    let request_id = response_request_id(&response);
    Err(shared_instances_status_error(
        operation,
        &method,
        path,
        status,
        request_id.as_deref(),
    ))
}

fn shared_instances_status_error(
    operation: &'static str,
    method: &Method,
    path: &str,
    status: StatusCode,
    request_id: Option<&str>,
) -> crate::Error {
    tracing::warn!(
        operation,
        method = method.as_str(),
        path,
        status = status.as_u16(),
        request_id = request_id.unwrap_or("none"),
        "Shared instances API request failed"
    );
    let message = format!(
        "shared instances API request {operation} {method} {path} failed with status {status}"
    );
    if status.is_server_error() {
        return anyhow!(message).context(ErrorKind::SharedInstancesApiError);
    }

    anyhow!(message)
}

pub(super) fn response_request_id(
    response: &reqwest::Response,
) -> Option<String> {
    let request_id = response.headers().get("x-request-id")?.to_str().ok()?;
    if request_id.is_empty()
        || request_id.len() > 128
        || !request_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || "-_.:".contains(character)
        })
    {
        return None;
    }

    Some(request_id.to_string())
}

pub(super) fn service_url(base_url: &str, path: &str) -> String {
    format!("{base_url}/v1{path}")
}

pub(super) fn service_base_url() -> &'static str {
    env!("SHARED_INSTANCES_API_BASE_URL").trim_end_matches('/')
}

pub(super) fn shared_instances_client(
    base_url: &str,
) -> &'static reqwest::Client {
    if base_url.starts_with("https://") {
        &REQWEST_CLIENT
    } else {
        &INSECURE_REQWEST_CLIENT
    }
}

pub(super) fn shared_instances_upload_client(
    base_url: &str,
) -> &'static reqwest::Client {
    if base_url.starts_with("https://") {
        &NO_TIMEOUT_REQWEST_CLIENT
    } else {
        &INSECURE_NO_TIMEOUT_REQWEST_CLIENT
    }
>>>>>>> modrinth/main
}
