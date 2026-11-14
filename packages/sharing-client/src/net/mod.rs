//! Reaching a Core: its `https://ip:port` address, TLS pinned to its certificate, and which IPs
//! are public.

mod address;
pub mod ip;
pub mod tls;

pub use address::{AddressError, CoreAddress};
