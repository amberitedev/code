//! Reaching a Core: its `https://ip:port` address, TLS pinned to its certificate, which IPs are
//! public, and the error body Cores, the store and the backend answer with.

mod address;
mod error_body;
pub mod ip;
pub mod tls;

pub use address::{AddressError, CoreAddress};
pub use error_body::ErrorBody;
