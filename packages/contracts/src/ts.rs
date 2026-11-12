//! TypeScript for the protocol, written with ts-rs. Each crate that declares Endpoints writes one
//! file into `packages/contracts/ts/` and checks it is fresh in its own tests.

use serde::Serialize;
use ts_rs::{Config, TS};

use crate::{
    Empty, Endpoint, Json, Raw, RequestBody, ResponseBody, Stream, Upgrade,
};

/// The one ts-rs config every crate generates with: JSON gives `number`, not ts-rs's `bigint`.
pub fn config() -> Config {
    Config::default().with_large_int("number")
}

/// The TypeScript type a body marker carries.
pub trait TsBody {
    fn ts(cfg: &Config) -> String;

    /// The type a caller sends, where it differs from the type it reads.
    fn request_ts(cfg: &Config) -> String {
        Self::ts(cfg)
    }
}

impl<T: TS> TsBody for Json<T> {
    fn ts(cfg: &Config) -> String {
        T::name(cfg)
    }
}

impl TsBody for Empty {
    fn ts(_: &Config) -> String {
        "void".into()
    }
}

impl TsBody for Raw {
    fn ts(_: &Config) -> String {
        "Blob".into()
    }
}

impl TsBody for Stream {
    fn ts(_: &Config) -> String {
        "Response".into()
    }

    fn request_ts(_: &Config) -> String {
        "Blob".into()
    }
}

impl<S: TS, R: TS> TsBody for Upgrade<S, R> {
    fn ts(cfg: &Config) -> String {
        format!("{{ receive: {}, send: {} }}", S::name(cfg), R::name(cfg))
    }
}

/// `E` as a TypeScript `Endpoint` value, to place in its module's object. The types it names must
/// be declared in the same file.
pub fn endpoint<E>() -> String
where
    E: Endpoint,
    E::Params: TS,
    E::Query: TS,
    E::Body: TsBody,
    E::Response: TsBody,
{
    let cfg = &config();
    format!(
        "{{ method: {}, path: {}, auth: {}, dangerous: {}, body: {}, response: {} }} as Endpoint<{}, {}, {}, {}>",
        json(&E::METHOD),
        json(&E::PATH),
        json(&E::AUTH),
        E::DANGEROUS,
        json(&<E::Body as RequestBody>::KIND),
        json(&<E::Response as ResponseBody>::KIND),
        E::Params::name(cfg),
        E::Query::name(cfg),
        E::Body::request_ts(cfg),
        E::Response::ts(cfg),
    )
}

fn json(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("plain values serialize")
}

/// A constant's TypeScript initializer, with its type where inference would widen it.
#[cfg(test)]
pub(crate) trait Const {
    fn ts(&self) -> String;
}

#[cfg(test)]
impl Const for u64 {
    fn ts(&self) -> String {
        format!(" = {self}")
    }
}

#[cfg(test)]
impl Const for usize {
    fn ts(&self) -> String {
        format!(" = {self}")
    }
}

#[cfg(test)]
impl Const for &str {
    fn ts(&self) -> String {
        format!(" = {}", json(self))
    }
}

#[cfg(test)]
impl<const N: usize> Const for [&str; N] {
    fn ts(&self) -> String {
        format!(": readonly string[] = {}", json(&self.as_slice()))
    }
}

#[cfg(test)]
pub(crate) fn constant(
    name: &str,
    docs: &[&str],
    value: &impl Const,
) -> String {
    let docs: String = docs
        .iter()
        .map(|line| format!(" *{}\n", line.trim_end()))
        .collect();
    format!("\n/**\n{docs} */\nexport const {name}{}\n", value.ts())
}

#[cfg(test)]
mod tests {
    use ts_rs::TS;

    use crate::{
        ApiError, Auth, BodyKind, Empty, Endpoint, ErrorCode, Json, Method,
        PROTOCOL_VERSION, backend, limits, rules, tickets,
    };

    const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ts/contracts.ts");

    const ENDPOINT: &str = "
/** A route, as `ContractClient` calls it. `_types` only carries its Rust types. */
export interface Endpoint<P = unknown, Q = unknown, B = unknown, R = unknown> {
\tmethod: Method
\tpath: string
\tauth: Auth
\tdangerous: boolean
\tbody: BodyKind
\tresponse: BodyKind
\treadonly _types?: { params: P; query: Q; body: B; response: R }
}
";

    fn file() -> String {
        let cfg = super::config();
        let declarations = [
            Method::decl(&cfg),
            Auth::decl(&cfg),
            BodyKind::decl(&cfg),
            ApiError::decl(&cfg),
            ErrorCode::decl(&cfg),
        ]
        .into_iter()
        .chain(tickets::ts_types())
        .map(|declaration| format!("\nexport {declaration}\n"))
        .collect::<String>();
        format!(
            "// Generated from packages/contracts by ts-rs. Do not edit.\n\
             // Regenerate: cargo test -p contracts -- --ignored write_ts\n\n\
             export const PROTOCOL_VERSION = {PROTOCOL_VERSION}\n{declarations}{ENDPOINT}{}{}{}{}{}",
            limits::ts(),
            rules::ts(),
            tickets::ts(),
            backend::cores::ts(),
            backend::social::ts(),
        )
    }

    #[test]
    fn ts_is_up_to_date() {
        let written = std::fs::read_to_string(PATH).unwrap_or_default();
        assert!(
            written.replace("\r\n", "\n") == file(),
            "{PATH} is out of date. Run: cargo test -p contracts -- --ignored write_ts"
        );
    }

    #[test]
    #[ignore = "writes the generated file; run on purpose"]
    fn write_ts() {
        std::fs::write(PATH, file()).unwrap();
    }

    struct ClearCache;

    impl Endpoint for ClearCache {
        const METHOD: Method = Method::Delete;
        const PATH: &'static str = "/v1/cache";
        const AUTH: Auth = Auth::Session;
        const DANGEROUS: bool = true;
        type Params = ();
        type Query = ();
        type Body = Empty;
        type Response = Json<u64>;
    }

    #[test]
    fn an_endpoint_is_the_object_contract_client_reads() {
        assert_eq!(
            super::endpoint::<ClearCache>(),
            r#"{ method: "DELETE", path: "/v1/cache", auth: "session", dangerous: true, body: "empty", response: "json" } as Endpoint<null, null, void, number>"#
        );
    }
}
