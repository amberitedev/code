//! The client for the backend's share routes (`/v1/instances/...` and `/v1/invites/...`), the
//! one both the App and Core use. The caller brings the base URL, the bearer token (an App
//! session, or a Core's key) and the `reqwest::Client`, so TLS, timeouts and the token source
//! stay its own.
//!
//! Every route reads a non-success answer the same way: 404 and 401 are [`Unavailable`] (see
//! [`Unavailable::from_status`]), and any other status is [`BackendError::Status`] with the
//! backend's error body. Routes whose 404 means something else say so where they are defined.

mod error;

pub use error::{BackendError, Unavailable};

use std::time::Duration;

use bedrock::net::ErrorBody;
use modrinth_content_management::icons::IconConfig;
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;

use crate::pool::Fault;
use crate::versions::{ManifestFile, ShareVersion, VersionContent};

/// The backend's share routes, reached as one caller.
#[derive(Clone)]
pub struct Backend {
    http: reqwest::Client,
    base_url: String,
    token: Option<String>,
    read_ticket: Option<String>,
    timeout: Option<Duration>,
}

/// A share's icon as the backend keeps it.
pub enum ShareIcon<'a> {
    /// Made in the icon editor: only its settings go up, and every client draws it.
    Drawn(&'a IconConfig),
    /// An uploaded image.
    Image(Vec<u8>),
}

/// The body of `POST /v1/instances/:id/versions`.
#[derive(Serialize)]
pub struct NewVersion<'a> {
    #[serde(flatten)]
    pub content: &'a VersionContent,
    pub external_files: &'a [ManifestFile],
    /// R, in hex.
    pub root: String,
    /// A Core of the owner that runs a server of this share, which the backend makes its home.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_core: Option<&'a str>,
}

impl Backend {
    /// `base_url` is the backend's root, without `/v1`. `token` is the bearer of every call.
    pub fn new(
        http: reqwest::Client,
        base_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            token: Some(token.into()),
            ..Self::anonymous(http, base_url)
        }
    }

    /// For the routes anyone may call, such as an invite's preview.
    pub fn anonymous(
        http: reqwest::Client,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            token: None,
            read_ticket: None,
            timeout: None,
        }
    }

    /// A Core key reading a share its server follows, with the read ticket the asking user's App
    /// got for this Core and share. The backend then reads it as that user, and only while they
    /// can read it.
    pub fn with_read_ticket(mut self, ticket: impl Into<String>) -> Self {
        self.read_ticket = Some(ticket.into());
        self
    }

    /// Gives up on each call after `timeout`, on top of whatever the client sets.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let mut request = self
            .http
            .request(method, format!("{}/v1{path}", self.base_url));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(ticket) = &self.read_ticket {
            request = request.header("X-Amberite-Read-Ticket", ticket);
        }
        if let Some(timeout) = self.timeout {
            request = request.timeout(timeout);
        }
        request
    }

    /// Sends `request` and returns the answer when it succeeded.
    async fn send(
        &self,
        operation: &'static str,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, BackendError> {
        let response =
            request.send().await.map_err(reqwest::Error::without_url)?;
        check(operation, response).await
    }

    async fn json<T: DeserializeOwned>(
        &self,
        operation: &'static str,
        request: reqwest::RequestBuilder,
    ) -> Result<T, BackendError> {
        let response = self.send(operation, request).await?;
        let text =
            response.text().await.map_err(reqwest::Error::without_url)?;
        serde_json::from_str(&text).map_err(|error| {
            tracing::warn!(
                operation,
                error_category = ?error.classify(),
                error_line = error.line(),
                error_column = error.column(),
                "Shared instances API returned an invalid JSON response"
            );
            BackendError::Decode { operation, error }
        })
    }

    async fn empty(
        &self,
        operation: &'static str,
        request: reqwest::RequestBuilder,
    ) -> Result<(), BackendError> {
        self.send(operation, request).await.map(drop)
    }

    // Shares.

    /// Creates a share owned by the caller and returns its id.
    pub async fn create_share(
        &self,
        name: &str,
    ) -> Result<String, BackendError> {
        let created: Created = self
            .json(
                "create_instance",
                self.request(Method::POST, "/instances")
                    .json(&json!({ "name": name })),
            )
            .await?;
        Ok(created.id)
    }

    /// The share of the calling Core's server `server_id`, created now unless it has one. Only
    /// a Core key may, for a server that follows no instance.
    pub async fn create_server_share(
        &self,
        name: &str,
        server_id: &str,
    ) -> Result<String, BackendError> {
        let created: Created = self
            .json(
                "create_instance",
                self.request(Method::POST, "/instances")
                    .json(&json!({ "name": name, "server_id": server_id })),
            )
            .await?;
        Ok(created.id)
    }

    /// `GET /instances/:id`, read as `T`: the App's view of the share.
    pub async fn share<T: DeserializeOwned>(
        &self,
        share_id: &str,
    ) -> Result<T, BackendError> {
        self.json(
            "get_instance",
            self.request(Method::GET, &format!("/instances/{share_id}")),
        )
        .await
    }

    pub async fn rename_share(
        &self,
        share_id: &str,
        name: &str,
    ) -> Result<(), BackendError> {
        self.empty(
            "update_instance",
            self.request(Method::PATCH, &format!("/instances/{share_id}"))
                .json(&json!({ "name": name })),
        )
        .await
    }

    /// Deletes the share. One already gone counts as deleted: 404 (no such share) and 410
    /// (`core_removed`: the Core was removed from its account, and its shares with it).
    pub async fn delete_share(
        &self,
        share_id: &str,
    ) -> Result<(), BackendError> {
        match self
            .empty(
                "delete_instance",
                self.request(Method::DELETE, &format!("/instances/{share_id}")),
            )
            .await
        {
            Err(error)
                if matches!(
                    error.status(),
                    Some(StatusCode::NOT_FOUND | StatusCode::GONE)
                ) =>
            {
                Ok(())
            }
            result => result,
        }
    }

    /// Sets the share's icon, or removes it with `None`.
    pub async fn set_share_icon(
        &self,
        share_id: &str,
        icon: Option<ShareIcon<'_>>,
    ) -> Result<(), BackendError> {
        let path = format!("/instances/{share_id}/icon");
        match icon {
            Some(ShareIcon::Drawn(config)) => {
                self.empty(
                    "upload_generated_instance_icon",
                    self.request(Method::PUT, &format!("{path}/generated"))
                        .json(config),
                )
                .await
            }
            Some(ShareIcon::Image(bytes)) => {
                // The backend serves the image with the type it was sent with.
                let mime = infer::get(&bytes)
                    .map_or("application/octet-stream", |kind| {
                        kind.mime_type()
                    });
                self.empty(
                    "upload_instance_icon",
                    self.request(Method::PUT, &path)
                        .header(reqwest::header::CONTENT_TYPE, mime)
                        .body(bytes),
                )
                .await
            }
            None => {
                self.empty(
                    "delete_instance_icon",
                    self.request(Method::DELETE, &path),
                )
                .await
            }
        }
    }

    /// The icon of the server a player came to the share through. `None` when it has none (404);
    /// with `etag`, a 304 answer when it is unchanged.
    pub async fn server_icon(
        &self,
        share_id: &str,
        etag: Option<&str>,
    ) -> Result<Option<reqwest::Response>, BackendError> {
        let mut request = self.request(
            Method::GET,
            &format!("/instances/{share_id}/server-icon"),
        );
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response =
            request.send().await.map_err(reqwest::Error::without_url)?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            StatusCode::NOT_MODIFIED => Ok(Some(response)),
            _ => check("get_instance_server_icon", response).await.map(Some),
        }
    }

    // Members.

    /// `GET /instances/:id/users`, read as `T`.
    pub async fn users<T: DeserializeOwned>(
        &self,
        share_id: &str,
    ) -> Result<T, BackendError> {
        self.json(
            "get_instance_users",
            self.request(Method::GET, &format!("/instances/{share_id}/users")),
        )
        .await
    }

    /// Invites `user_ids` and returns those whose privacy settings refuse the invitation.
    pub async fn add_users(
        &self,
        share_id: &str,
        user_ids: &[String],
    ) -> Result<Vec<String>, BackendError> {
        #[derive(Deserialize)]
        struct Added {
            failed: Vec<String>,
        }
        let added: Added = self
            .json(
                "add_instance_users",
                self.request(
                    Method::POST,
                    &format!("/instances/{share_id}/users"),
                )
                .json(&json!({ "user_ids": user_ids })),
            )
            .await?;
        Ok(added.failed)
    }

    pub async fn remove_users(
        &self,
        share_id: &str,
        user_ids: &[String],
    ) -> Result<(), BackendError> {
        self.empty(
            "remove_instance_users",
            self.request(
                Method::DELETE,
                &format!("/instances/{share_id}/users"),
            )
            .json(&json!({ "user_ids": user_ids })),
        )
        .await
    }

    // Versions.

    /// A version of the share, or the latest one without `version`: the newest ready one, or
    /// the one uploading while none is ready. A 404 is a share without such a version as much
    /// as a share that is gone.
    pub async fn version(
        &self,
        share_id: &str,
        version: Option<i32>,
    ) -> Result<ShareVersion, BackendError> {
        let path = match version {
            Some(version) => {
                format!("/instances/{share_id}/versions/{version}")
            }
            None => format!("/instances/{share_id}/versions"),
        };
        self.json("get_instance_version", self.request(Method::GET, &path))
            .await
    }

    /// The latest version members can install, or `None` while there is none.
    pub async fn latest_ready(
        &self,
        share_id: &str,
    ) -> Result<Option<ShareVersion>, BackendError> {
        match self.version(share_id, None).await {
            Ok(version) => Ok(version.ready.then_some(version)),
            Err(BackendError::Unavailable {
                reason: Unavailable::Deleted,
                ..
            }) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Creates a version. The answer is the version, with a store grant until it is ready.
    pub async fn create_version(
        &self,
        share_id: &str,
        body: &NewVersion<'_>,
    ) -> Result<ShareVersion, BackendError> {
        self.json(
            "create_instance_version",
            self.request(
                Method::POST,
                &format!("/instances/{share_id}/versions"),
            )
            .json(body),
        )
        .await
    }

    /// Tells the backend a holder served a bad file of `version`.
    pub async fn report_fault(
        &self,
        share_id: &str,
        version: i32,
        fault: &Fault,
    ) -> Result<(), BackendError> {
        self.empty(
            "report_pool_failure",
            self.request(
                Method::POST,
                &format!("/instances/{share_id}/versions/{version}/reports"),
            )
            .json(
                &json!({ "core_id": fault.core, "kind": fault.kind.as_str() }),
            ),
        )
        .await
    }

    // Invites.

    /// `GET /instances/:id/invites`, read as `T`.
    pub async fn invites<T: DeserializeOwned>(
        &self,
        share_id: &str,
    ) -> Result<T, BackendError> {
        self.json(
            "get_instance_invites",
            self.request(
                Method::GET,
                &format!("/instances/{share_id}/invites"),
            ),
        )
        .await
    }

    /// Creates an invite link and returns its id.
    pub async fn create_invite(
        &self,
        share_id: &str,
        max_age_seconds: i32,
        max_uses: i32,
    ) -> Result<String, BackendError> {
        let created: Created = self
            .json(
                "create_instance_invite",
                self.request(
                    Method::POST,
                    &format!("/instances/{share_id}/invites"),
                )
                .json(&json!({ "max_age": max_age_seconds, "max_uses": max_uses })),
            )
            .await?;
        Ok(created.id)
    }

    pub async fn delete_invite(
        &self,
        share_id: &str,
        invite_id: &str,
    ) -> Result<(), BackendError> {
        self.empty(
            "delete_instance_invite",
            self.request(
                Method::DELETE,
                &format!("/instances/{share_id}/invites/{invite_id}"),
            ),
        )
        .await
    }

    /// Joins the share through an invite link. Joining a share one already has is done.
    pub async fn accept_invite(
        &self,
        share_id: &str,
        invite_id: &str,
    ) -> Result<(), BackendError> {
        match self
            .empty(
                "accept_instance_invite",
                self.request(
                    Method::POST,
                    &format!("/instances/{share_id}/invites/{invite_id}"),
                ),
            )
            .await
        {
            Err(BackendError::Status {
                status: StatusCode::BAD_REQUEST,
                body,
                ..
            }) if body.description.contains("already has access") => Ok(()),
            result => result,
        }
    }

    /// Accepts the caller's pending invite. False when there is none (404).
    pub async fn accept_pending_invite(
        &self,
        share_id: &str,
    ) -> Result<bool, BackendError> {
        match self
            .empty(
                "accept_pending_instance_invite",
                self.request(
                    Method::POST,
                    &format!("/instances/{share_id}/invites/pending"),
                ),
            )
            .await
        {
            Ok(()) => Ok(true),
            Err(BackendError::Unavailable {
                reason: Unavailable::Deleted,
                ..
            }) => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub async fn decline_pending_invite(
        &self,
        share_id: &str,
    ) -> Result<(), BackendError> {
        self.empty(
            "decline_pending_instance_invite",
            self.request(
                Method::DELETE,
                &format!("/instances/{share_id}/invites/pending"),
            ),
        )
        .await
    }

    /// An invite's preview with the share's latest ready version, read as `T`. Anyone may ask.
    pub async fn invite<T: DeserializeOwned>(
        &self,
        invite_id: &str,
    ) -> Result<T, BackendError> {
        self.json(
            "get_instance_invite",
            self.request(
                Method::GET,
                &format!("/invites/{invite_id}?version=true"),
            ),
        )
        .await
    }
}

#[derive(Deserialize)]
struct Created {
    #[serde(alias = "instance_id")]
    id: String,
}

/// `response` when it succeeded, otherwise its error with the body it carried. Logs only
/// `operation`, since paths can hold invite ids.
async fn check(
    operation: &'static str,
    response: reqwest::Response,
) -> Result<reqwest::Response, BackendError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let request_id = request_id(&response);
    let text = response.bytes().await.unwrap_or_default();
    let body = ErrorBody::parse(&text).unwrap_or_default();
    tracing::warn!(
        operation,
        status = status.as_u16(),
        code = body.code(),
        request_id = request_id.as_deref().unwrap_or("none"),
        "Shared instances API request failed"
    );
    Err(match Unavailable::from_status(status) {
        Some(reason) => BackendError::Unavailable { reason, body },
        None => BackendError::Status {
            operation,
            status,
            body,
        },
    })
}

/// The backend's request id, when it is one worth logging.
fn request_id(response: &reqwest::Response) -> Option<String> {
    let request_id = response.headers().get("x-request-id")?.to_str().ok()?;
    (!request_id.is_empty()
        && request_id.len() <= 128
        && request_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || "-_.:".contains(character)
        }))
    .then(|| request_id.to_owned())
}
