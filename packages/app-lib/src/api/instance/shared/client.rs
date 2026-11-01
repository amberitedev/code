use super::install::*;
use super::*;
use crate::ErrorKind;
use crate::self_hosted::pool::PoolAccess;
use crate::self_hosted::pool::store::PoolCore;
use anyhow::{Context, anyhow, bail};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub(super) enum SharedInstancesRequestAuth {
    AmberiteSession,
    None,
}

impl SharedInstancesRequestAuth {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::AmberiteSession => "amberite_session",
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
pub(super) struct CreateInstanceResponse {
    #[serde(alias = "instance_id")]
    pub(super) id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct CreateInstanceInviteResponse {
    pub(super) id: String,
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
    pub(super) managers: Vec<InstanceInviteManagerResponse>,
    #[serde(default)]
    pub(super) version: Option<InstanceVersionResponse>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum InstanceInviteManagerResponse {
    User { id: String },
    Server { name: String, icon: Option<String> },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(from = "RawInstanceVersionResponse")]
pub(super) struct InstanceVersionResponse {
    pub(super) version: i32,
    pub(super) modrinth_ids: Vec<String>,
    pub(super) ready: bool,
    pub(super) external_files: Vec<ExternalFileResponse>,
    pub(super) modpack_id: Option<String>,
    pub(super) backtrack_version: Option<String>,
    pub(super) removed_files: Vec<SharedInstanceRemovedFile>,
    pub(super) game_version: String,
    pub(super) loader: ModLoader,
    pub(super) loader_version: String,
    /// R, the hash that names this version in the pool.
    pub(super) root: Option<String>,
    /// Where members fetch the files, once the version is ready.
    pub(super) pool: Option<RemotePoolAccess>,
}

impl InstanceVersionResponse {
    /// Gives each file the pool access of the version it came from, since a member's config
    /// files are gathered from several versions.
    fn with_pool_access(mut self, shared_instance_id: &str) -> Self {
        if let (Some(pool), Some(root)) = (&self.pool, &self.root) {
            let access = Arc::new(PoolAccess {
                root: root.clone(),
                holders: pool.holders.clone(),
                ticket: pool.ticket.clone(),
                shared_instance_id: shared_instance_id.to_string(),
                version: self.version,
            });
            for file in &mut self.external_files {
                file.pool = Some(Arc::clone(&access));
            }
        }
        self
    }
}

#[derive(Deserialize)]
struct RawInstanceVersionResponse {
    version: i32,
    #[serde(default)]
    modrinth_ids: Vec<String>,
    ready: bool,
    #[serde(default)]
    external_files: Vec<ExternalFileResponse>,
    #[serde(default)]
    modpack_id: Option<String>,
    #[serde(default)]
    backtrack_version: Option<String>,
    #[serde(default)]
    removed_files: Vec<SharedInstanceRemovedFile>,
    game_version: String,
    loader: String,
    loader_version: String,
    #[serde(default)]
    root: Option<String>,
    #[serde(default)]
    pool: Option<RemotePoolAccess>,
}

impl From<RawInstanceVersionResponse> for InstanceVersionResponse {
    /// Servers without a client loader (vanilla, Paper, Purpur) are installed
    /// as Fabric so players can add client-side mods on top.
    fn from(raw: RawInstanceVersionResponse) -> Self {
        let (loader, loader_version) = match ModLoader::from_string(&raw.loader)
        {
            ModLoader::Vanilla => (ModLoader::Fabric, String::new()),
            loader => (loader, raw.loader_version),
        };
        Self {
            version: raw.version,
            modrinth_ids: raw.modrinth_ids,
            ready: raw.ready,
            external_files: raw.external_files,
            modpack_id: raw.modpack_id,
            backtrack_version: raw.backtrack_version,
            removed_files: raw.removed_files,
            game_version: raw.game_version,
            loader,
            loader_version,
            root: raw.root,
            pool: raw.pool,
        }
    }
}

impl InstanceVersionResponse {
    pub(super) fn validate_removed_files(&self) -> crate::Result<()> {
        if self.modpack_id.as_deref().is_none_or(str::is_empty)
            && !self.removed_files.is_empty()
        {
            bail!(ErrorKind::InputError(
                "removed modpack files require a modpack id".to_string(),
            ));
        }
        for removed in &self.removed_files {
            removed.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct RemotePoolAccess {
    pub(super) holders: Vec<PoolCore>,
    pub(super) ticket: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct ExternalFileResponse {
    pub(super) file_name: String,
    pub(super) file_type: String,
    /// `pool:<C>` for pool files, which nothing fetches.
    pub(super) url: Option<String>,
    #[serde(default)]
    pub(super) file_size: Option<i64>,
    #[serde(default)]
    pub(super) sha1: Option<String>,
    /// P, which a member checks after decrypting.
    #[serde(default)]
    pub(super) sha256: Option<String>,
    /// C, for files in the pool.
    #[serde(default)]
    pub(super) object: Option<String>,
    #[serde(default)]
    pub(super) object_size: Option<u64>,
    #[serde(default)]
    pub(super) key: Option<String>,
    /// Filled in from the version, see [`InstanceVersionResponse::with_pool_access`].
    #[serde(skip)]
    pub(super) pool: Option<Arc<PoolAccess>>,
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

pub(super) async fn create_remote_instance(
    name: String,
    state: &State,
) -> crate::Result<CreateInstanceResponse> {
    request_json(
        "create_instance",
        Method::POST,
        "/instances",
        Some(json!({ "name": name })),
        state,
    )
    .await
}

pub(super) async fn delete_remote_instance(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    let operation = "delete_instance";
    let method = Method::DELETE;
    let path = format!("/instances/{shared_instance_id}");
    let response =
        send_request(operation, method.clone(), &path, None, state).await?;

    if response.status().is_success()
        || matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::GONE)
    {
        return Ok(());
    }

    shared_instances_request_error(operation, method, &path, response).await
}

pub(super) async fn update_remote_instance(
    shared_instance_id: &str,
    name: String,
    state: &State,
) -> crate::Result<()> {
    let operation = "update_instance";
    let method = Method::PATCH;
    let path = format!("/instances/{shared_instance_id}");
    let response = send_request(
        operation,
        method.clone(),
        &path,
        Some(json!({ "name": name })),
        state,
    )
    .await?;

    if response.status().is_success() {
        return Ok(());
    }

    if response.status() == StatusCode::METHOD_NOT_ALLOWED {
        let request_id = response_request_id(&response);
        tracing::warn!(
            operation,
            method = method.as_str(),
            path,
            status = StatusCode::METHOD_NOT_ALLOWED.as_u16(),
            request_id = request_id.as_deref().unwrap_or("none"),
            "Shared instances API does not support remote instance updates; skipping name sync"
        );
        return Ok(());
    }

    shared_instances_request_error(operation, method, &path, response).await
}

pub(super) async fn get_remote_instance_access(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<SharedInstanceRemoteResponse<RemoteInstanceResponse>> {
    let operation = "get_instance";
    let method = Method::GET;
    let path = format!("/instances/{shared_instance_id}");
    let response =
        send_request(operation, method.clone(), &path, None, state).await?;

    if let Some(reason) =
        SharedInstanceUnavailableReason::from_status(response.status())
    {
        if reason == SharedInstanceUnavailableReason::AccessRevoked
            && !active_amberite_session_is_configured(state).await?
        {
            bail!(crate::ErrorKind::NotLoggedIn);
        }

        return Ok(SharedInstanceRemoteResponse::Unavailable(reason));
    }

    if !response.status().is_success() {
        return shared_instances_request_error(
            operation, method, &path, response,
        )
        .await;
    }

    let instance = decode_json_response::<RemoteInstanceResponse>(
        operation, method, &path, response,
    )
    .await?;
    if instance.quarantine {
        return Ok(SharedInstanceRemoteResponse::Unavailable(
            SharedInstanceUnavailableReason::Quarantined,
        ));
    }

    Ok(SharedInstanceRemoteResponse::Available(instance))
}

pub(super) async fn update_remote_instance_icon(
    shared_instance_id: &str,
    icon_path: Option<&str>,
    state: &State,
) -> crate::Result<()> {
    let path = format!("/instances/{shared_instance_id}/icon");
    let Some(icon_path) = icon_path else {
        return request_empty(
            "delete_instance_icon",
            Method::DELETE,
            &path,
            None,
            state,
        )
        .await;
    };

    let bytes = crate::util::io::read(icon_path).await?;
    let operation = "upload_instance_icon";
    let method = Method::PUT;
    let response =
        send_bytes_request(operation, method.clone(), &path, bytes, state)
            .await?;

    if response.status().is_success() {
        return Ok(());
    }

    shared_instances_request_error(operation, method, &path, response).await
}

pub(super) async fn get_remote_users(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<SharedInstanceUsers> {
    let users = request_json::<RemoteSharedInstanceUsersResponse>(
        "get_instance_users",
        Method::GET,
        &format!("/instances/{shared_instance_id}/users"),
        None,
        state,
    )
    .await?;

    Ok(users.into_shared_users())
}

pub(super) async fn get_remote_server_icon(
    instance_id: &str,
    etag: Option<&str>,
    state: &State,
) -> crate::Result<reqwest::Response> {
    let access_token = shared_clients_session()
        .and_then(|session| session.access_token)
        .ok_or(crate::ErrorKind::NotLoggedIn)?;
    let _permit = state.api_semaphore.0.acquire().await?;
    let base_url = service_base_url()?;
    let path = format!("/instances/{instance_id}/server-icon");
    let mut request = shared_instances_client(&base_url)
        .get(service_url(&base_url, &path))
        .bearer_auth(access_token)
        .timeout(std::time::Duration::from_secs(15));
    if let Some(etag) = etag {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    request
        .send()
        .await
        .map_err(|error| error.without_url())
        .context(ErrorKind::SharedInstancesApiError)
}

pub(super) async fn get_latest_remote_version(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<InstanceVersionResponse> {
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
) -> crate::Result<SharedInstanceRemoteResponse<InstanceVersionResponse>> {
    get_latest_remote_version_optional_unavailable_with_auth(
        shared_instance_id,
        state,
        SharedInstancesRequestAuth::AmberiteSession,
    )
    .await
}

pub(super) async fn get_latest_remote_version_optional_unavailable_with_auth(
    shared_instance_id: &str,
    state: &State,
    auth: SharedInstancesRequestAuth,
) -> crate::Result<SharedInstanceRemoteResponse<InstanceVersionResponse>> {
    Ok(
        match request_json_optional_unavailable::<InstanceVersionResponse>(
            "get_latest_instance_version",
            Method::GET,
            &format!("/instances/{shared_instance_id}/versions"),
            None,
            state,
            auth,
        )
        .await?
        {
            SharedInstanceRemoteResponse::Available(version) => {
                SharedInstanceRemoteResponse::Available(
                    version.with_pool_access(shared_instance_id),
                )
            }
            unavailable => unavailable,
        },
    )
}

/// Returns `None` for versions that never became ready.
pub(super) async fn get_remote_version(
    shared_instance_id: &str,
    version: i32,
    state: &State,
) -> crate::Result<Option<InstanceVersionResponse>> {
    match request_json_optional_unavailable::<InstanceVersionResponse>(
        "get_instance_version",
        Method::GET,
        &format!("/instances/{shared_instance_id}/versions/{version}"),
        None,
        state,
        SharedInstancesRequestAuth::AmberiteSession,
    )
    .await?
    {
        SharedInstanceRemoteResponse::Available(version) => {
            Ok(Some(version.with_pool_access(shared_instance_id)))
        }
        SharedInstanceRemoteResponse::Unavailable(
            SharedInstanceUnavailableReason::Deleted,
        ) => Ok(None),
        SharedInstanceRemoteResponse::Unavailable(reason) => {
            Err(shared_instance_unavailable_error(reason))
        }
    }
}

pub(super) async fn add_remote_users(
    shared_instance_id: &str,
    user_ids: Vec<String>,
    state: &State,
) -> crate::Result<()> {
    /// The accounts the backend did not invite because their privacy settings refuse it.
    #[derive(Deserialize)]
    struct AddUsersResponse {
        failed: Vec<String>,
    }

    let response: AddUsersResponse = request_json(
        "add_instance_users",
        Method::POST,
        &format!("/instances/{shared_instance_id}/users"),
        Some(json!({ "user_ids": user_ids })),
        state,
    )
    .await?;
    if !response.failed.is_empty() {
        bail!(ErrorKind::InputError(format!(
            "{} of the invited accounts do not allow sharing invitations in their privacy settings",
            response.failed.len()
        )));
    }
    Ok(())
}

pub(super) async fn remove_remote_users(
    shared_instance_id: &str,
    user_ids: Vec<String>,
    state: &State,
) -> crate::Result<()> {
    request_empty(
        "remove_instance_users",
        Method::DELETE,
        &format!("/instances/{shared_instance_id}/users"),
        Some(json!({ "user_ids": user_ids })),
        state,
    )
    .await
}

pub(super) async fn accept_pending_remote_invite(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<bool> {
    let operation = "accept_pending_instance_invite";
    let method = Method::POST;
    let path = format!("/instances/{shared_instance_id}/invites/pending");
    let response =
        send_request(operation, method.clone(), &path, None, state).await?;

    match response.status() {
        StatusCode::OK | StatusCode::NO_CONTENT => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        status if status.is_success() => Ok(true),
        _ => {
            shared_instances_request_error(operation, method, &path, response)
                .await
        }
    }
}

pub(super) async fn accept_shared_instance_invite(
    shared_instance_id: &str,
    invite_id: &str,
    state: &State,
) -> crate::Result<()> {
    let path = format!("/instances/{shared_instance_id}/invites/{invite_id}");
    let log_path = "/instances/:instance_id/invites/:invite_id";
    let operation = "accept_instance_invite";
    let method = Method::POST;

    let response = send_request_with_auth_and_log_path(
        operation,
        method.clone(),
        &path,
        log_path,
        None,
        state,
        SharedInstancesRequestAuth::AmberiteSession,
    )
    .await?;
    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let request_id = response_request_id(&response);
    let body = response
        .text()
        .await
        .map_err(reqwest::Error::without_url)
        .unwrap_or_default();
    if status == StatusCode::BAD_REQUEST && body.contains("already has access")
    {
        return Ok(());
    }

    tracing::warn!(
        operation,
        method = method.as_str(),
        path = log_path,
        status = status.as_u16(),
        request_id = request_id.as_deref().unwrap_or("none"),
        "Shared instances API request failed"
    );
    bail!(
        "shared instances API request {operation} {method} {log_path} failed with status {status}"
    )
}

pub(super) async fn delete_remote_invite(
    shared_instance_id: &str,
    invite_id: &str,
    state: &State,
) -> crate::Result<()> {
    let path = format!("/instances/{shared_instance_id}/invites/{invite_id}");
    request_empty_with_log_path(
        "delete_instance_invite",
        Method::DELETE,
        &path,
        "/instances/:instance_id/invites/:invite_id",
        None,
        state,
    )
    .await
}

pub(super) async fn get_remote_invites(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<Vec<InstanceInviteResponse>> {
    request_json(
        "get_instance_invites",
        Method::GET,
        &format!("/instances/{shared_instance_id}/invites"),
        None,
        state,
    )
    .await
}

pub(super) async fn get_shared_instance_invite_info(
    invite_id: &str,
    state: &State,
) -> crate::Result<InstanceInviteInfoResponse> {
    let operation = "get_instance_invite";
    let method = Method::GET;
    let path = format!("/invites/{invite_id}?version=true");
    let log_path = "/invites/:invite_id";
    let response = send_request_with_auth_and_log_path(
        operation,
        method.clone(),
        &path,
        log_path,
        None,
        state,
        SharedInstancesRequestAuth::None,
    )
    .await?;
    if !response.status().is_success() {
        return shared_instances_request_error(
            operation, method, log_path, response,
        )
        .await;
    }

    decode_json_response_with_log_path(
        operation, method, log_path, response, true,
    )
    .await
}

pub(super) async fn decline_pending_remote_invite(
    shared_instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    request_empty(
        "decline_pending_instance_invite",
        Method::DELETE,
        &format!("/instances/{shared_instance_id}/invites/pending"),
        None,
        state,
    )
    .await
}

pub(super) async fn request_json<T>(
    operation: &'static str,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
    state: &State,
) -> crate::Result<T>
where
    T: DeserializeOwned,
{
    let response =
        request(operation, method.clone(), path, body, state).await?;
    decode_json_response(operation, method, path, response).await
}

pub(super) async fn request_json_optional_unavailable<T>(
    operation: &'static str,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
    state: &State,
    auth: SharedInstancesRequestAuth,
) -> crate::Result<SharedInstanceRemoteResponse<T>>
where
    T: DeserializeOwned,
{
    let response = send_request_with_auth(
        operation,
        method.clone(),
        path,
        body,
        state,
        auth,
    )
    .await?;
    decode_optional_unavailable(operation, method, path, response, state, auth)
        .await
}

/// Tells the backend a holder served a bad file. Best effort: the install goes on either way.
pub(crate) async fn report_pool_failure(
    shared_instance_id: &str,
    version: i32,
    core_id: &str,
    kind: &str,
) {
    let result = async {
        let state = State::get().await?;
        request_empty(
            "report_pool_failure",
            Method::POST,
            &format!(
                "/instances/{shared_instance_id}/versions/{version}/reports"
            ),
            Some(json!({ "core_id": core_id, "kind": kind })),
            &state,
        )
        .await
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(
            shared_instance_id,
            version,
            core_id,
            kind,
            error = %error,
            "Could not report a shared file failure"
        );
    }
}

async fn decode_optional_unavailable<T>(
    operation: &'static str,
    method: Method,
    path: &str,
    response: reqwest::Response,
    state: &State,
    auth: SharedInstancesRequestAuth,
) -> crate::Result<SharedInstanceRemoteResponse<T>>
where
    T: DeserializeOwned,
{
    if let Some(reason) =
        SharedInstanceUnavailableReason::from_status(response.status())
    {
        if reason == SharedInstanceUnavailableReason::AccessRevoked
            && matches!(auth, SharedInstancesRequestAuth::AmberiteSession)
            && !active_amberite_session_is_configured(state).await?
        {
            tracing::warn!(
                operation,
                method = method.as_str(),
                path,
                status = response.status().as_u16(),
                "Shared instances API returned unauthorized while Modrinth auth is unavailable"
            );
            bail!(crate::ErrorKind::NotLoggedIn);
        }

        tracing::warn!(
            operation,
            method = method.as_str(),
            path,
            status = response.status().as_u16(),
            "Shared instances API resource is unavailable"
        );
        return Ok(SharedInstanceRemoteResponse::Unavailable(reason));
    }

    if !response.status().is_success() {
        return shared_instances_request_error(
            operation, method, path, response,
        )
        .await;
    }

    decode_json_response(operation, method, path, response)
        .await
        .map(SharedInstanceRemoteResponse::Available)
}

pub(super) async fn active_amberite_session_is_configured(
    _state: &State,
) -> crate::Result<bool> {
    Ok(shared_clients_session()
        .is_some_and(|session| session.access_token.is_some()))
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
        SharedInstancesRequestAuth::AmberiteSession,
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
        SharedInstancesRequestAuth::AmberiteSession,
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
    let base_url = service_base_url()?;
    let url = service_url(&base_url, path);
    send_bytes_request_to_url(operation, method, path, &url, body, None, state)
        .await
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
    let service_origin = url::Url::parse(&service_base_url()?)
        .context("invalid shared instances API base URL")?
        .origin();
    let upload_origin = url::Url::parse(url)
        .context("invalid shared instances upload URL")?
        .origin();
    if service_origin != upload_origin {
        bail!("shared instances upload URL has an unexpected origin");
    }

    let session = shared_clients_session()
        .and_then(|session| session.access_token)
        .ok_or(crate::ErrorKind::NotLoggedIn)?;
    let _permit = state.api_semaphore.0.acquire().await?;

    tracing::debug!(
        operation,
        method = method.as_str(),
        path,
        url = %url,
        auth = SharedInstancesRequestAuth::AmberiteSession.label(),
        has_body = true,
        "Sending shared instances API request"
    );

    let mut request = shared_instances_upload_client(url)
        .request(method.clone(), url)
        .bearer_auth(session)
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
    let amberite_session =
        if matches!(auth, SharedInstancesRequestAuth::AmberiteSession) {
            Some(
                shared_clients_session()
                    .and_then(|session| session.access_token)
                    .ok_or(crate::ErrorKind::NotLoggedIn)?,
            )
        } else {
            None
        };
    let _permit = state.api_semaphore.0.acquire().await?;
    let base_url = service_base_url()?;
    let url = service_url(&base_url, path);
    let log_url = service_url(&base_url, log_path);
    let mut request =
        shared_instances_client(&base_url).request(method.clone(), &url);

    match auth {
        SharedInstancesRequestAuth::AmberiteSession => {
            let access_token = amberite_session
                .expect("Amberite session credentials were loaded");
            request = request.bearer_auth(access_token);
        }
        SharedInstancesRequestAuth::None => {}
    }

    tracing::debug!(
        operation,
        method = method.as_str(),
        path = log_path,
        url = %log_url,
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
    tracing::warn!(
        operation,
        method = method.as_str(),
        path,
        status = status.as_u16(),
        request_id = request_id.as_deref().unwrap_or("none"),
        "Shared instances API request failed"
    );
    let message = format!(
        "shared instances API request {operation} {method} {path} failed with status {status}"
    );
    if status.is_server_error() {
        return Err(
            anyhow!(message).context(ErrorKind::SharedInstancesApiError)
        );
    }

    Err(anyhow!(message))
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

pub(super) fn service_base_url() -> crate::Result<String> {
    shared_clients_session()
        .map(|session| session.base_url)
        .context("Amberite shared clients service is not configured")
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
}
