use bedrock::net::ErrorBody;
use reqwest::StatusCode;

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
    /// The one reading of a share route's status: 404 is [`Self::Deleted`], 401
    /// [`Self::AccessRevoked`], and every other status is not about the share's availability.
    pub fn from_status(status: StatusCode) -> Option<Self> {
        match status {
            StatusCode::NOT_FOUND => Some(Self::Deleted),
            StatusCode::UNAUTHORIZED => Some(Self::AccessRevoked),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    /// A share route answered 404 or 401. See [`Unavailable::from_status`].
    #[error("{reason}")]
    Unavailable {
        reason: Unavailable,
        body: ErrorBody,
    },
    /// Any other answer but success, with the error body it carried (empty when it was none).
    #[error(
        "shared instances API request {operation} failed with status {status}"
    )]
    Status {
        operation: &'static str,
        status: StatusCode,
        body: ErrorBody,
    },
    #[error(
        "shared instances API request {operation} sent unreadable JSON: {error}"
    )]
    Decode {
        operation: &'static str,
        error: serde_json::Error,
    },
    #[error("could not reach the shared instances API: {0}")]
    Http(#[from] reqwest::Error),
}

impl BackendError {
    /// The status the backend answered with, when it answered.
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Self::Unavailable {
                reason: Unavailable::Deleted,
                ..
            } => Some(StatusCode::NOT_FOUND),
            Self::Unavailable {
                reason: Unavailable::AccessRevoked,
                ..
            } => Some(StatusCode::UNAUTHORIZED),
            Self::Status { status, .. } => Some(*status),
            Self::Decode { .. } | Self::Http(_) => None,
        }
    }

    /// The error body the backend answered with, when it answered with one.
    pub fn body(&self) -> Option<&ErrorBody> {
        match self {
            Self::Unavailable { body, .. } | Self::Status { body, .. } => {
                Some(body)
            }
            Self::Decode { .. } | Self::Http(_) => None,
        }
    }
}
