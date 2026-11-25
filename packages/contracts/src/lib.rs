//! The protocol between the App, Cores, their stores and the backend. A shape is written once here
//! and generated to TypeScript in `ts/protocol.ts`, `ts/backend.ts` and `ts/core.ts`, which
//! `cargo test -p contracts` keeps fresh. Regenerate them with `WRITE_TS=1 node scripts/dev-runner.ts test contracts`.

use std::marker::PhantomData;

use bytes::Bytes;
use serde::{Serialize, de::DeserializeOwned};

pub mod backend;
#[cfg(feature = "client")]
pub mod client;
pub mod core;
pub mod error;
mod hash;
mod instance;
pub mod limits;
pub mod rules;
#[cfg(feature = "server")]
pub mod server;
mod side;
pub mod store;
pub mod tickets;
#[cfg(feature = "export-ts")]
pub mod ts;

pub use error::{ApiError, ErrorCode};
pub use hash::Hash;
pub use instance::{ModLoader, ReleaseChannel};
pub use side::Side;

/// Bumped only when old and new sides can no longer talk to each other. Releases do not touch it.
pub const PROTOCOL_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
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
    /// A Core answers only once the work is done, which can take any time, so the TypeScript
    /// client sets no timeout. A Core serves it outside its request timeout.
    const UNTIMED: bool = false;
    /// Fills the path's `{name}` segments; `()` when it has none.
    type Params: Serialize + DeserializeOwned + Send;
    type Query: Serialize + DeserializeOwned + Send;
    type Body: RequestBody;
    type Response: ResponseBody;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
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

/// Declares routes: per line a unit struct and its [`Endpoint`], and with `export-ts` a
/// `ts_module()` that gives them as the TypeScript object `group`. Every crate that declares
/// routes uses this one form.
///
/// ```ignore
/// contracts::endpoints! {
///     /// The TypeScript object's doc.
///     group core;
///
///     /// A route's doc.
///     GetServer: Get "/servers/{id}", Session, ServerId, (), Empty => Json<CoreServer>;
///     DeleteServer: Delete "/servers/{id}", Session, ServerId, (), Empty => Empty, dangerous;
/// }
/// ```
///
/// A line is `Name: Method "path", Auth, Params, Query, Body => Response`, then its flags:
/// `dangerous` when the route needs a fresh ticket ([`Endpoint::DANGEROUS`]), `untimed` when a
/// Core answers it only once done ([`Endpoint::UNTIMED`]), and for an engine route its [`Reach`]:
/// `shared` or `client_only`. Any other flag does not compile.
#[macro_export]
macro_rules! endpoints {
    (@flag dangerous) => {};
    (@flag untimed) => {};
    (@flag shared) => {};
    (@flag client_only) => {};
    (@flag $other:ident) => {
        compile_error!(concat!("unknown endpoint flag `", stringify!($other), "`"));
    };
    (@dangerous) => { false };
    (@dangerous dangerous $($rest:ident)*) => { true };
    (@dangerous $other:ident $($rest:ident)*) => { $crate::endpoints!(@dangerous $($rest)*) };
    (@untimed) => { false };
    (@untimed untimed $($rest:ident)*) => { true };
    (@untimed $other:ident $($rest:ident)*) => { $crate::endpoints!(@untimed $($rest)*) };
    (@reach) => { None };
    (@reach shared $($rest:ident)*) => { Some($crate::Reach::Shared) };
    (@reach client_only $($rest:ident)*) => { Some($crate::Reach::ClientOnly) };
    (@reach $other:ident $($rest:ident)*) => { $crate::endpoints!(@reach $($rest)*) };
    (
        $(#[doc = $group_doc:literal])*
        group $group:ident;
        $(
            $(#[doc = $doc:literal])*
            $name:ident: $method:ident $path:literal, $auth:ident, $params:ty, $query:ty, $body:ty => $response:ty $(, $flag:ident)*;
        )+
    ) => {
        $(
            $($crate::endpoints!(@flag $flag);)*

            $(#[doc = $doc])*
            pub struct $name;

            impl $crate::Endpoint for $name {
                const METHOD: $crate::Method = $crate::Method::$method;
                const PATH: &'static str = $path;
                const AUTH: $crate::Auth = $crate::Auth::$auth;
                const DANGEROUS: bool = $crate::endpoints!(@dangerous $($flag)*);
                const UNTIMED: bool = $crate::endpoints!(@untimed $($flag)*);
                type Params = $params;
                type Query = $query;
                type Body = $body;
                type Response = $response;
            }
        )+

        /// These routes' TypeScript object.
        #[cfg(feature = "export-ts")]
        pub fn ts_module() -> $crate::ts::Module {
            $crate::ts::Module {
                name: stringify!($group),
                docs: &[$($group_doc),*],
                endpoints: vec![$((stringify!($name), $crate::ts::endpoint::<$name>())),+],
            }
        }

        /// The routes whose params, query, body or response names a host path.
        #[cfg(feature = "export-ts")]
        pub fn host_paths() -> Vec<&'static str> {
            [$((stringify!($name), $crate::ts::names_host_path::<$name>())),+]
                .into_iter()
                .filter_map(|(name, host)| host.then_some(name))
                .collect()
        }

        /// Each route's name, method and path, whether it needs a fresh ticket or a Core answers
        /// it only once done, and where an engine route is served.
        pub const ROUTES: &[$crate::Route] = &[$($crate::Route {
            name: stringify!($name),
            method: $crate::Method::$method,
            path: $path,
            dangerous: $crate::endpoints!(@dangerous $($flag)*),
            untimed: $crate::endpoints!(@untimed $($flag)*),
            reach: $crate::endpoints!(@reach $($flag)*),
        }),+];
    };
}

/// One route of an [`endpoints!`] group, for tests that walk every route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub name: &'static str,
    pub method: Method,
    pub path: &'static str,
    pub dangerous: bool,
    pub untimed: bool,
    /// `None` for a route that is not the engine's.
    pub reach: Option<Reach>,
}

/// Which hosts serve an engine route. A Core serves its servers' engine routes to remote users,
/// so a route that takes or answers a host path, reaches the whole engine, or runs code on the
/// host is the App's alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// The App and every Core.
    Shared,
    /// Only the App.
    ClientOnly,
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
