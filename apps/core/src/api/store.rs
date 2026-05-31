//! The persistence and membership boundaries the message layer depends on.
//! `presentation`/`infrastructure` supply implementations; the layer only calls
//! these traits.

use async_trait::async_trait;

use super::audience::Endpoint;
use super::envelope::Envelope;
use super::error::ApiCommError;
use super::ids::{GroupId, InstanceId, MessageId, UserId};
use super::relay_store::StoredMessage;
use super::status::DeliveryStatus;

/// Durable storage for relay messages. One row = one message destined for one
/// recipient, tracked through its [`DeliveryStatus`] lifecycle.
#[async_trait]
pub trait RelayStore: Send + Sync {
    /// Persist a sealed envelope for a specific recipient at `Pending`.
    async fn enqueue(&self, recipient: &Endpoint, envelope: &Envelope)
        -> Result<(), ApiCommError>;

    /// Persist a pre-built row (used by the generic, app-addressed publish path).
    async fn insert(&self, message: &StoredMessage) -> Result<(), ApiCommError>;

    /// Non-expired pending messages for a recipient, oldest first.
    async fn pending(&self, recipient_id: &str) -> Result<Vec<StoredMessage>, ApiCommError>;

    /// Look up a single message by id.
    async fn get(&self, id: &MessageId) -> Result<Option<StoredMessage>, ApiCommError>;

    /// Transition a message to a new status, optionally attaching a result/error.
    async fn mark(
        &self,
        id: &MessageId,
        recipient_id: &str,
        status: DeliveryStatus,
        result: Option<serde_json::Value>,
        error: Option<String>,
    ) -> Result<(), ApiCommError>;
}

/// Answers audience questions. Core knows who is synced to what; the layer asks
/// rather than embedding that knowledge.
#[async_trait]
pub trait MembershipResolver: Send + Sync {
    async fn instance_members(
        &self,
        instance: &InstanceId,
        except: Option<&UserId>,
    ) -> Result<Vec<Endpoint>, ApiCommError>;

    async fn group_members(
        &self,
        group: &GroupId,
        except: Option<&UserId>,
    ) -> Result<Vec<Endpoint>, ApiCommError>;
}
