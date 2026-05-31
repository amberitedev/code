//! Who a message is for. Core resolves a high-level [`Audience`] into concrete
//! [`Endpoint`]s via a [`crate::api::MembershipResolver`].

use serde::{Deserialize, Serialize};

use super::ids::{CoreId, GroupId, InstanceId, UserId};

/// A single concrete recipient.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "id")]
pub enum Endpoint {
    User(UserId),
    Core(CoreId),
}

impl Endpoint {
    /// The recipient id as stored in the relay table, regardless of kind.
    pub fn recipient_id(&self) -> &str {
        match self {
            Endpoint::User(id) => id.as_str(),
            Endpoint::Core(id) => id.as_str(),
        }
    }
}

/// A high-level description of who should receive a message. Expanded into
/// concrete [`Endpoint`]s at distribution time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Audience {
    /// Exactly one endpoint.
    Direct { endpoint: Endpoint },
    /// Everyone synced to an instance, optionally excluding one user.
    InstanceMembers {
        instance: InstanceId,
        except: Option<UserId>,
    },
    /// Every member of a friend group, optionally excluding one user.
    Group {
        group: GroupId,
        except: Option<UserId>,
    },
}
