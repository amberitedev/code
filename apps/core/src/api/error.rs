//! Errors the API message layer can return.

use super::ids::MessageId;
use super::kind::MessageKind;
use super::policy::Route;

#[derive(Debug, thiserror::Error)]
pub enum ApiCommError {
    #[error("message kind {kind} cannot travel via route {}", .route.wire())]
    WrongRoute { kind: MessageKind, route: Route },

    #[error("route {} is not implemented", .0.wire())]
    RouteNotImplemented(Route),

    #[error("payload serialization failed: {0}")]
    Payload(#[from] serde_json::Error),

    #[error("relay store error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("membership resolution failed: {0}")]
    Membership(String),

    #[error("unknown message id: {0}")]
    UnknownMessage(MessageId),
}
