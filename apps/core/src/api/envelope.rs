//! The wire [`Envelope`]: a sealed, route-stamped message ready to store or send.
//! Built only by sealing a typed [`Message`], which copies the message's policy
//! onto the envelope so the two cannot disagree.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::error::ApiCommError;
use super::ids::{MessageId, UserId};
use super::kind::MessageKind;
use super::message::Message;
use super::policy::{Ack, Durability, Route};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub id: MessageId,
    pub kind: MessageKind,
    pub version: u16,
    pub route: Route,
    pub ack: Ack,
    pub durability: Durability,
    pub origin: Option<UserId>,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Envelope {
    /// Seal a typed message into a transport-ready envelope. The single entry
    /// point for building one; policy constants are stamped here.
    pub fn seal<M: Message>(
        message: &M,
        origin: Option<UserId>,
    ) -> Result<Self, ApiCommError> {
        let created_at = Utc::now();
        let expires_at = match M::DURABILITY {
            Durability::Durable => Some(
                created_at + Duration::seconds(M::default_ttl_secs() as i64),
            ),
            Durability::Ephemeral => None,
        };

        Ok(Self {
            id: MessageId::generate(),
            kind: M::KIND,
            version: M::VERSION,
            route: M::ROUTE,
            ack: M::ACK,
            durability: M::DURABILITY,
            origin,
            payload: serde_json::to_value(message)?,
            created_at,
            expires_at,
        })
    }

    /// Decode the payload back into its typed message. Fails if the envelope's
    /// kind does not match the requested type.
    pub fn decode<M: Message>(&self) -> Result<M, ApiCommError> {
        if self.kind != M::KIND {
            return Err(ApiCommError::WrongRoute {
                kind: self.kind,
                route: self.route,
            });
        }
        Ok(serde_json::from_value(self.payload.clone())?)
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        matches!(self.expires_at, Some(deadline) if now >= deadline)
    }

    pub fn kind_wire(&self) -> &'static str {
        self.kind.wire()
    }

    pub fn expects_ack(&self) -> bool {
        !matches!(self.ack, Ack::None)
    }
}
