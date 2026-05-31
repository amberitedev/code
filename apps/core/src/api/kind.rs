//! The closed set of message kinds Core knows how to send. The dotted wire names
//! exist only here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageKind {
    /// An instance's sync settings changed.
    InstanceSyncChanged,
    /// An instance's mod list changed; synced members reconcile.
    InstanceModsChanged,
    /// A Core registering itself for pairing with a user account.
    CorePairingRegistration,
    /// One user inviting another to be friends.
    FriendInvite,
}

impl MessageKind {
    pub fn wire(self) -> &'static str {
        match self {
            MessageKind::InstanceSyncChanged => "instance.sync.changed",
            MessageKind::InstanceModsChanged => "instance.mods.changed",
            MessageKind::CorePairingRegistration => "core.pairing.registration",
            MessageKind::FriendInvite => "friend.invite",
        }
    }
}

impl std::fmt::Display for MessageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.wire())
    }
}
