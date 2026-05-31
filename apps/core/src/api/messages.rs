//! The concrete messages Core sends. Each is a payload struct plus one [`Message`]
//! impl that locks in its route/ack/durability.

use serde::{Deserialize, Serialize};

use super::ids::{CoreId, InstanceId, UserId};
use super::kind::MessageKind;
use super::message::Message;
use super::policy::{Ack, Durability, Route};

/// A reference to a single mod in an instance's mod list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModRef {
    pub project_id: String,
    pub version_id: Option<String>,
}

/// An owner changed an instance's mod list. Posted to Core, fanned out to everyone
/// synced to the instance. Durable so offline members still get it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceModsChanged {
    pub instance: InstanceId,
    pub mods: Vec<ModRef>,
}

impl Message for InstanceModsChanged {
    const KIND: MessageKind = MessageKind::InstanceModsChanged;
    const ROUTE: Route = Route::CorePost;
    const ACK: Ack = Ack::Received;
    const DURABILITY: Durability = Durability::Durable;
}

/// An instance's sync configuration changed. Posted to Core and distributed to
/// affected members.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSyncChanged {
    pub instance: InstanceId,
    pub sync_enabled: bool,
    pub members: Vec<UserId>,
}

impl Message for InstanceSyncChanged {
    const KIND: MessageKind = MessageKind::InstanceSyncChanged;
    const ROUTE: Route = Route::CorePost;
    const ACK: Ack = Ack::Received;
    const DURABILITY: Durability = Durability::Durable;
}

/// A Core registers itself so a user account can pair with it. One of the few
/// flows allowed to use Convex.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorePairingRegistration {
    pub core: CoreId,
    pub user: UserId,
    pub pairing_code: String,
}

impl Message for CorePairingRegistration {
    const KIND: MessageKind = MessageKind::CorePairingRegistration;
    const ROUTE: Route = Route::ConvexRelay;
    const ACK: Ack = Ack::Processed;
    const DURABILITY: Durability = Durability::Durable;
}

/// One user invites another to be friends. The other user may be offline on a
/// different Core, so this is the second flow permitted to use Convex.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendInvite {
    pub from: UserId,
    pub to: UserId,
}

impl Message for FriendInvite {
    const KIND: MessageKind = MessageKind::FriendInvite;
    const ROUTE: Route = Route::ConvexRelay;
    const ACK: Ack = Ack::Received;
    const DURABILITY: Durability = Durability::Durable;

    fn default_ttl_secs() -> u64 {
        60 * 60 * 24 * 14
    }
}
