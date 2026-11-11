//! The protocol between the App, Cores, their stores and the backend. A shape is written once here
//! and generated to TypeScript in `ts/contracts.ts`, which `cargo test -p contracts` keeps fresh.
//! Regenerate it with `cargo test -p contracts -- --ignored write_ts`.

use std::marker::PhantomData;

use bytes::Bytes;
use serde::{Serialize, de::DeserializeOwned};

pub mod backend;
#[cfg(feature = "client")]
pub mod client;
pub mod error;
pub mod limits;
pub mod rules;
#[cfg(feature = "server")]
pub mod server;
mod side;
pub mod tickets;
#[cfg(feature = "ts")]
pub mod ts;

pub use error::{ApiError, ErrorCode};
pub use side::Side;

/// Bumped only when old and new sides can no longer talk to each other. Releases do not touch it.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Auth {
    None,
    Session,
    CoreKey,
    Account,
    /// The store's read ticket, sent in place of the bearer token. Not the fresh ticket a
    /// dangerous Core route needs: that is [`Endpoint::DANGEROUS`].
    Ticket,
    Host,
}

/// One route: a unit struct per method and path.
pub trait Endpoint {
    const METHOD: Method;
    /// An axum path template, such as `/v1/instances/{id}/versions`.
    const PATH: &'static str;
    const AUTH: Auth;
    /// The TypeScript client attaches a fresh ticket, which Core's `Dangerous` extractor requires.
    const DANGEROUS: bool = false;
    /// Fills the path's `{name}` segments; `()` when it has none.
    type Params: Serialize + DeserializeOwned + Send;
    type Query: Serialize + DeserializeOwned + Send;
    type Body: RequestBody;
    type Response: ResponseBody;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum BodyKind {
    Json,
    Empty,
    Raw,
    Stream,
    Upgrade,
}

pub trait RequestBody {
    const KIND: BodyKind;
}

pub trait ResponseBody {
    const KIND: BodyKind;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Json<T>(pub T);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Empty;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raw {
    pub content_type: String,
    pub bytes: Bytes,
}

/// A body sent or read as it arrives rather than held whole, such as an upload or a resumable
/// download.
#[derive(Clone, Copy, Debug)]
pub struct Stream;

/// A WebSocket on which the server sends `S` and receives `R`.
pub struct Upgrade<S, R>(PhantomData<fn() -> (S, R)>);

impl<T> RequestBody for Json<T> {
    const KIND: BodyKind = BodyKind::Json;
}

impl RequestBody for Empty {
    const KIND: BodyKind = BodyKind::Empty;
}

impl RequestBody for Raw {
    const KIND: BodyKind = BodyKind::Raw;
}

impl RequestBody for Stream {
    const KIND: BodyKind = BodyKind::Stream;
}

impl<T> ResponseBody for Json<T> {
    const KIND: BodyKind = BodyKind::Json;
}

impl ResponseBody for Empty {
    const KIND: BodyKind = BodyKind::Empty;
}

impl ResponseBody for Raw {
    const KIND: BodyKind = BodyKind::Raw;
}

impl ResponseBody for Stream {
    const KIND: BodyKind = BodyKind::Stream;
}

impl<S, R> ResponseBody for Upgrade<S, R> {
    const KIND: BodyKind = BodyKind::Upgrade;
}

/// Declares documented `pub const`s and, for the tests, their TypeScript.
macro_rules! consts {
    ($($(#[doc = $doc:literal])+ $name:ident: $ty:ty = $value:expr;)+) => {
        $($(#[doc = $doc])+ pub const $name: $ty = $value;)+

        #[cfg(test)]
        pub(crate) fn ts() -> String {
            [$(crate::ts::constant(stringify!($name), &[$($doc),+], &$name)),+].concat()
        }
    };
}
pub(crate) use consts;
