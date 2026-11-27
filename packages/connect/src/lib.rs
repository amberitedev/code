//! Direct connections between Amberite programs with no port forwarded, and what the rest of
//! Amberite shares about addresses and certificates.
//!
//! Two programs behind home routers cannot simply connect: each router drops packets it did not
//! ask for. So both learn where the other might be from the backend (an introduction), and send
//! toward each other at once. Each router then takes the other side's packets for answers to its
//! own, and lets them in. Every byte after that goes directly between the two; nothing is ever
//! relayed. Two routers that give every destination another port cannot be connected this way,
//! and [`Endpoint::dial`] then fails with [`NoPath`].
//!
//! - A program is known by its certificate ([`PeerId`]), never by an address. The connection is
//!   QUIC with TLS 1.3 both ways: the dialer accepts only the certificate it was told, and the
//!   listener learns the dialer's id from the one it shows.
//! - A listener is invisible to anyone it was not introduced to: its socket drops their packets
//!   before QUIC sees them (`socket`). This computer's own networks are the exception a host
//!   may allow ([`Gate`]).
//! - Each use of a connection is a protocol named in the handshake (an ALPN), with a stream per
//!   request.
//!
//! The hosts carry the introduction: a Core holds a socket to the backend and an App asks the
//! backend for one. This crate never talks to the backend.

mod endpoint;
pub mod ip;
mod socket;
pub mod stun;
pub mod tls;

pub use endpoint::{
    Connection, Endpoint, Incoming, MAX_CANDIDATES, NoPath, Stream,
    on_own_network,
};
pub use socket::Gate;
pub use tls::{Identity, PeerId};

/// The Core's HTTP API, a request per stream.
pub const API: &[u8] = b"amberite/api/1";

#[cfg(test)]
mod tests;
