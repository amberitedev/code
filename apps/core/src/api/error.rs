//! The error body of every route. The shape matches the backend's errors, so the App reads both
//! the same way. The HTTP status follows from the code.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ApiError {
    pub error: ErrorCode,
    /// Ready to show.
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No session, or it expired or was revoked. Sign in again with a ticket.
    Unauthorized,
    /// The session's user has no role on the server that allows this.
    Forbidden,
    /// A dangerous action without the `X-Amberite-Ticket` header.
    TicketRequired,
    /// Bad signature, wrong algorithm, wrong issuer, or a ticket for another user.
    TicketInvalid,
    /// The ticket is for another Core.
    TicketWrongCore,
    TicketExpired,
    /// The ticket was already used once.
    TicketReused,
    /// The ticket is outside the clock leeway in a way only a wrong clock explains.
    ClockWrong,
    /// Signed in correctly, but not on this Core's access list.
    NotInvited,
    NotFound,
    /// Too many requests, such as more than 10 sign-ins a minute from one address.
    RateLimited,
    InvalidRequest,
    /// The thing is in a state that does not allow this, such as starting a running server.
    Conflict,
    /// Something failed on the Core.
    Internal,
}

impl ApiError {
    pub fn new(error: ErrorCode, description: impl Into<String>) -> Self {
        Self {
            error,
            description: description.into(),
        }
    }
}

impl ErrorCode {
    pub fn status(self) -> StatusCode {
        match self {
            Self::Unauthorized
            | Self::TicketRequired
            | Self::TicketInvalid
            | Self::TicketWrongCore
            | Self::TicketExpired
            | Self::TicketReused
            | Self::ClockWrong => StatusCode::UNAUTHORIZED,
            Self::Forbidden | Self::NotInvited => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::InvalidRequest => StatusCode::BAD_REQUEST,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.error.status(), Json(self)).into_response()
    }
}
