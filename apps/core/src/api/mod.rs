//! Core API — the typed message layer.
//!
//! Owns the vocabulary for everything Core sends or distributes: message kinds,
//! their fixed routing/ack/durability policy, the wire [`Envelope`], the
//! [`Distributor`] fan-out engine, and the [`RelayStore`] persistence boundary.
//! `presentation` is where these are physically served over HTTP.

#![allow(unused_imports)]

mod audience;
mod connection;
mod distributor;
mod envelope;
mod error;
mod ids;
mod kind;
mod message;
mod messages;
mod policy;
mod relay_store;
mod status;
mod store;

pub use audience::{Audience, Endpoint};
pub use connection::{
    ConnectionHandshakeRequest, ConnectionHandshakeResponse,
    ConnectionRejectReason, HANDSHAKE_PROTOCOL,
};
pub use distributor::{Distribution, Distributor};
pub use envelope::Envelope;
pub use error::ApiCommError;
pub use ids::{CoreId, GroupId, InstanceId, MessageId, UserId};
pub use kind::MessageKind;
pub use message::Message;
pub use messages::{
    CorePairingRegistration, FriendInvite, InstanceModsChanged,
    InstanceSyncChanged, ModRef,
};
pub use policy::{Ack, Durability, Route};
pub use relay_store::{SqliteRelayStore, StoredMessage};
pub use status::DeliveryStatus;
pub use store::{MembershipResolver, RelayStore};
