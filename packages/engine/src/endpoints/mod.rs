//! The engine's Endpoints, over its own types. `crate::routes` serves them; their TypeScript goes
//! to `packages/contracts/ts/engine.ts` with the engine's other wire types.

pub mod cache;
pub mod groups;
pub mod instances;
pub mod jre;
pub mod loading;
pub mod logs;
pub mod metadata;
pub mod process;
pub mod store;
pub mod tags;
pub mod worlds;

/// Declares each route as a unit struct and, for the tests, the entries of its TypeScript module
/// object. Every engine route is called with the host's session.
macro_rules! endpoints {
    ($(
        $(#[doc = $doc:literal])*
        $name:ident: $method:ident $path:literal, $params:ty, $query:ty, $body:ty => $response:ty;
    )+) => {
        $(
            $(#[doc = $doc])*
            pub struct $name;

            impl contracts::Endpoint for $name {
                const METHOD: contracts::Method = contracts::Method::$method;
                const PATH: &'static str = $path;
                const AUTH: contracts::Auth = contracts::Auth::Session;
                type Params = $params;
                type Query = $query;
                type Body = $body;
                type Response = $response;
            }
        )+

        #[cfg(test)]
        pub(crate) fn ts_entries() -> String {
            [$(format!("\t{}: {},\n", stringify!($name), contracts::ts::endpoint::<$name>())),+]
                .concat()
        }
    };
}
use endpoints;
