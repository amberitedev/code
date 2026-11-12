//! Paired Cores: the proxy address that reaches each, and this device's session there. See
//! `crate::cores`.

use contracts::{Auth, Empty, Endpoint, Json, Method};
use serde::{Deserialize, Serialize};

/// Returns the base URL on the App's Core proxy that reaches the Core, trusting only the
/// certificate with `fingerprint` and signing with `account`'s session there, when it has one.
pub struct ConnectCore;

impl Endpoint for ConnectCore {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/cores/connect";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<CoreTarget>;
    type Response = Json<String>;
}

/// Trades a ticket for a session on the Core, named after this computer, and keeps the token for
/// `account`. Returns the session, never the token.
pub struct SignInToCore;

impl Endpoint for SignInToCore {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/cores/sessions";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<NewCoreSession>;
    type Response = Json<CoreSession>;
}

/// Ends `account`'s session on the Core, when it answers, and forgets the token either way.
pub struct SignOutOfCore;

impl Endpoint for SignOutOfCore {
    const METHOD: Method = Method::Delete;
    const PATH: &'static str = "/app/cores/sessions";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = CoreSessionKey;
    type Body = Empty;
    type Response = Empty;
}

/// Whether `account` holds a session for the Core. It may have expired there.
pub struct CheckCoreSession;

impl Endpoint for CheckCoreSession {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/cores/sessions";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = CoreSessionKey;
    type Body = Empty;
    type Response = Json<bool>;
}

/// A paired Core as `account` reaches it: its `https://host:port` address and the SHA-256 of its
/// certificate.
#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CoreTarget {
    pub core_id: String,
    pub account: String,
    pub address: String,
    pub fingerprint: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct NewCoreSession {
    pub core_id: String,
    pub account: String,
    pub address: String,
    pub fingerprint: String,
    pub ticket: String,
}

/// The session as the Core describes it.
#[derive(Serialize)]
#[serde(transparent)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CoreSession(
    #[cfg_attr(test, ts(type = "unknown"))] pub serde_json::Value,
);

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CoreSessionKey {
    pub core_id: String,
    pub account: String,
}
