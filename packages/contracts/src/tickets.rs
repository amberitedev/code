//! Tickets: short Ed25519-signed JWTs (`alg: EdDSA`, `kid` naming the key) that the backend issues
//! and a Core or its store checks with the keys [`crate::backend::cores::Keys`] serves. Every
//! ticket names its `kind`, and each kind is taken on its own routes only. Signing stores nothing.

use serde::{Deserialize, Serialize, de::DeserializeOwned};

crate::consts! {
    /// The `iss` of every ticket the backend signs.
    TICKET_ISSUER: &str = "amberite";
    /// The header a Core reading a share's versions for a user sends the read ticket that user's
    /// App got for it in. Never the query, which request logs keep.
    READ_TICKET_HEADER: &str = "x-amberite-read-ticket";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TicketKind {
    Session,
    Store,
    Fetch,
    Pull,
    Read,
}

/// The claims of one kind of ticket, so a verifier can require the kind before it reads the rest.
pub trait Ticket: DeserializeOwned {
    const KIND: TicketKind;
}

/// The App signs in to one Core as one user, or authorizes one dangerous action there. Five
/// minutes, one use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SessionTicket {
    pub iss: String,
    pub kind: TicketKind,
    /// The user id.
    pub sub: String,
    pub name: String,
    pub avatar_url: Option<String>,
    /// The Core it is for.
    pub aud: String,
    /// Unix seconds.
    pub iat: i64,
    /// Unix seconds.
    pub exp: i64,
    /// Random one-time id. The Core records it in the same write that uses the ticket.
    pub jti: String,
}

/// The owner's App uploads one version of at most `bytes` to the Core in `aud`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoreTicket {
    pub iss: String,
    pub kind: TicketKind,
    pub aud: Audience,
    /// The version's hash.
    pub root: String,
    pub bytes: u64,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

/// A member's App reads one version from any of the Cores in `aud`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FetchTicket {
    pub iss: String,
    pub kind: TicketKind,
    pub aud: Audience,
    /// The version's hash.
    pub root: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

/// The Core in `aud` copies one version from `source`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PullTicket {
    pub iss: String,
    pub kind: TicketKind,
    pub aud: Audience,
    /// The version's hash.
    pub root: String,
    pub source: PullSource,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

/// The App lets one Core read one share for its user, so a server can follow it. The backend
/// checks this kind itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReadTicket {
    pub iss: String,
    pub kind: TicketKind,
    /// The user id.
    pub sub: String,
    /// The Core it is for.
    pub aud: String,
    /// The share id.
    pub share: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

/// The Core a ticket is for, or the Cores it may be used at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum Audience {
    One(String),
    Many(Vec<String>),
}

/// Where a pull copies from, and a fetch ticket for it there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PullSource {
    pub core: StoreHost,
    pub ticket: String,
}

/// A Core whose store holds versions, as the App and other Cores reach it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoreHost {
    pub id: String,
    /// `https://ip:port` on its own network.
    pub address: String,
    /// `https://ip:port` from the internet, once known.
    pub public_address: Option<String>,
    /// SHA-256 of its certificate, lowercase hex. The only certificate a caller accepts there.
    pub cert_fingerprint: String,
}

impl Ticket for SessionTicket {
    const KIND: TicketKind = TicketKind::Session;
}

impl Ticket for StoreTicket {
    const KIND: TicketKind = TicketKind::Store;
}

impl Ticket for FetchTicket {
    const KIND: TicketKind = TicketKind::Fetch;
}

impl Ticket for PullTicket {
    const KIND: TicketKind = TicketKind::Pull;
}

impl Ticket for ReadTicket {
    const KIND: TicketKind = TicketKind::Read;
}

/// The backend's ticket keys as a JWK set. A key stays listed for a day after it is replaced.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PublicKeys {
    pub keys: Vec<PublicKey>,
}

/// One Ed25519 public key as a JWK.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PublicKey {
    /// Always `OKP`.
    pub kty: String,
    /// Always `Ed25519`.
    pub crv: String,
    /// The id a ticket names in its `kid` header.
    pub kid: String,
    /// The key, base64url.
    pub x: String,
}

#[cfg(test)]
pub(crate) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![
        TicketKind::decl(&cfg),
        SessionTicket::decl(&cfg),
        StoreTicket::decl(&cfg),
        FetchTicket::decl(&cfg),
        PullTicket::decl(&cfg),
        ReadTicket::decl(&cfg),
        Audience::decl(&cfg),
        PullSource::decl(&cfg),
        StoreHost::decl(&cfg),
        PublicKeys::decl(&cfg),
        PublicKey::decl(&cfg),
    ]
}
