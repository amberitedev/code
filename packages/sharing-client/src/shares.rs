//! What the App and Core read the same way from the backend's shared instance routes
//! (`contracts::backend::sharing`), which they call with `contracts::client::call`, and their
//! twins a Core calls with its key (`contracts::backend::cores`).

use std::time::Duration;

use contracts::backend::cores::DeleteServerShare;
use contracts::backend::sharing::{DeleteSharedInstance, SharedInstanceId};
use contracts::client::{Body, CallError, Remote, Reply, call};
use contracts::{Empty, Endpoint};

/// Who calls the share routes: an account with its session, or a Core with its key, which acts
/// for its owner on routes of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caller {
    Account,
    Core,
}

/// `A` as an account calls it, or `C`, the same call on a Core's own route.
pub async fn call_as<A, C>(
    caller: Caller,
    remote: &Remote<'_>,
    params: &A::Params,
    query: &A::Query,
    body: <A::Body as Body>::Input,
) -> Result<<A::Response as Reply>::Output, CallError>
where
    A: Endpoint,
    A::Body: Body,
    A::Response: Reply,
    C: Endpoint<
            Params = A::Params,
            Query = A::Query,
            Body = A::Body,
            Response = A::Response,
        >,
{
    match caller {
        Caller::Account => call::<A>(remote, params, query, body).await,
        Caller::Core => call::<C>(remote, params, query, body).await,
    }
}

/// A [`Remote`] that owns its client and token, for calls made after the code that set it up
/// returns, such as an install job's.
#[derive(Clone)]
pub struct OwnedRemote {
    pub http: reqwest::Client,
    pub base: String,
    pub bearer: Option<String>,
    pub timeout: Option<Duration>,
    /// Who `bearer` is, which picks the share routes it calls.
    pub caller: Caller,
}

impl OwnedRemote {
    pub fn remote(&self) -> Remote<'_> {
        Remote {
            http: &self.http,
            base: &self.base,
            bearer: self.bearer.as_deref(),
            ticket: None,
            timeout: self.timeout,
        }
    }
}

/// Why the backend would not give a share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Unavailable {
    /// 404: the share is gone, has no such version, or belongs to another backend.
    #[error("The shared instance was deleted.")]
    Deleted,
    /// 401: the token no longer reaches the share.
    #[error("You no longer have access to the shared instance.")]
    AccessRevoked,
}

impl Unavailable {
    /// The one reading of a share route's failure: 404 is [`Self::Deleted`], 401
    /// [`Self::AccessRevoked`], and every other answer is not about the share's availability.
    pub fn of(error: &CallError) -> Option<Self> {
        match status(error)? {
            404 => Some(Self::Deleted),
            401 => Some(Self::AccessRevoked),
            _ => None,
        }
    }
}

/// The status the backend answered with, when it answered.
pub fn status(error: &CallError) -> Option<u16> {
    match error {
        CallError::Api { status, .. }
        | CallError::Decode { status, .. }
        | CallError::RangeIgnored { status } => Some(*status),
        CallError::Http(error) => error.status().map(|status| status.as_u16()),
        CallError::Params { .. } | CallError::Transport(_) => None,
    }
}

/// Deletes the share. One already gone counts as deleted: 404 (no such share) and 410
/// `core_removed` (the Core was removed from its account, and its shares with it).
pub async fn delete_share(
    caller: Caller,
    remote: &Remote<'_>,
    share_id: &str,
) -> Result<(), CallError> {
    let id = SharedInstanceId {
        id: share_id.to_owned(),
    };
    let deleted = call_as::<DeleteSharedInstance, DeleteServerShare>(
        caller,
        remote,
        &id,
        &(),
        Empty,
    );
    match deleted.await {
        Err(error) if matches!(status(&error), Some(404 | 410)) => Ok(()),
        result => result,
    }
}
