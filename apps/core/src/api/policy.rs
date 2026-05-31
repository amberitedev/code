//! The three policy dimensions every message declares. Enums, never strings; the
//! wire spelling lives here and nowhere else.

use serde::{Deserialize, Serialize};

/// How a message is allowed to travel.
///
/// `CoreDirect`/`CorePost` cover almost everything. `ConvexRelay` is reserved for
/// the few always-online flows (friend invites, core pairing). `PeerToPeer` has no
/// executor yet and is rejected at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Synchronous request/response to a Core HTTP endpoint.
    CoreDirect,
    /// Post a fact to Core; Core decides recipients and distributes.
    CorePost,
    /// Deliver to one recipient via the durable relay queue.
    CoreRelay,
    /// Deliver through Convex. Friend invites and core pairing only.
    ConvexRelay,
    /// Stays on this machine.
    Local,
    /// Reserved: direct client-to-client. Not implemented.
    PeerToPeer,
}

impl Route {
    pub fn wire(self) -> &'static str {
        match self {
            Route::CoreDirect => "core-direct",
            Route::CorePost => "core-post",
            Route::CoreRelay => "core-relay",
            Route::ConvexRelay => "convex-relay",
            Route::Local => "local",
            Route::PeerToPeer => "peer-to-peer",
        }
    }

    pub fn is_implemented(self) -> bool {
        !matches!(self, Route::PeerToPeer)
    }

    pub fn uses_convex(self) -> bool {
        matches!(self, Route::ConvexRelay)
    }
}

/// What confirmation the sender expects back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ack {
    /// Fire and forget.
    None,
    /// Confirm receipt.
    Received,
    /// Confirm processing completed (result may be attached).
    Processed,
}

impl Ack {
    pub fn wire(self) -> &'static str {
        match self {
            Ack::None => "none",
            Ack::Received => "received",
            Ack::Processed => "processed",
        }
    }
}

/// Whether a message must outlive a closed laptop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Durability {
    /// Dropped if it cannot be delivered now.
    Ephemeral,
    /// Stored and retried until acknowledged, expired, or failed.
    Durable,
}

impl Durability {
    pub fn is_persistent(self) -> bool {
        matches!(self, Durability::Durable)
    }
}
