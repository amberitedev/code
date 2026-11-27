//! Reaching a Core: its `https://ip:port` address, and from `connect`, TLS pinned to its
//! certificate and which IPs are public.

mod address;
pub use connect::{ip, tls};

pub use address::{AddressError, CoreAddress};
