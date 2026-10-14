//! The error body of every Amberite-only route. The shape matches the backend's errors, so the
//! App reads both the same way.

#![allow(dead_code)] // Wired in step 3: access, sessions, and activity routes.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
pub struct ApiError {
    pub error: ErrorCode,
    /// Ready to show.
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
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
}
