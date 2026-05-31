//! The [`Message`] trait: a payload type plus the compile-time constants that fix
//! how it may travel. [`super::Envelope::seal`] copies these onto the envelope, so
//! a message cannot be sent on the wrong route or ack policy.

use serde::{de::DeserializeOwned, Serialize};

use super::kind::MessageKind;
use super::policy::{Ack, Durability, Route};

pub trait Message: Serialize + DeserializeOwned {
    const KIND: MessageKind;
    const ROUTE: Route;
    const ACK: Ack;
    const DURABILITY: Durability;
    const VERSION: u16 = 1;

    /// How long (seconds) a durable copy is kept before expiring. Defaults to one day.
    fn default_ttl_secs() -> u64 {
        60 * 60 * 24
    }
}
