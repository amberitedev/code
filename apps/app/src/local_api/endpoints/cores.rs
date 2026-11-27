//! Paired Cores: the proxy address that reaches each, and this device's session there. See
//! `crate::cores`.

use std::net::SocketAddr;

use contracts::{Empty, Json, backend::cores::Dialer, core::CoreSession};
use serde::{Deserialize, Serialize};

contracts::endpoints! {
    group pairedCores;

    /// Connects to the Core that holds the certificate with `fingerprint`, unless a connection is
    /// open, and returns the base URL on the App's Core proxy that reaches it, signing with
    /// `account`'s session there when it has one. A Core that is not on this computer's network
    /// is not reached until the backend introduced this device: the answer then carries the
    /// `dialer` to introduce, and the call is made again with where the backend says the Core is.
    ConnectCore: Post "/app/cores/connect", None, (), (), Json<CoreTarget> => Json<CoreRoute>;
    /// Trades a ticket for a session on a connected Core, named after this computer, and keeps
    /// the token for `account`. Returns the session, never the token.
    SignInToCore: Post "/app/cores/sessions", None, (), (), Json<NewCoreSession> => Json<CoreSession>;
    /// Ends `account`'s session on the Core, when it answers, and forgets the token either way.
    SignOutOfCore: Delete "/app/cores/sessions", None, (), CoreSessionKey, Empty => Empty;
    /// Whether `account` holds a session for the Core. It may have expired there.
    CheckCoreSession: Get "/app/cores/sessions", None, (), CoreSessionKey, Empty => Json<bool>;
}

/// A paired Core as `account` reaches it.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreTarget {
    pub core_id: String,
    pub account: String,
    /// The SHA-256 of the Core's certificate.
    pub fingerprint: String,
    /// Where the Core is on its own network, `https://ip:port`.
    pub address: String,
    /// Where the backend's introduction says the Core is.
    pub introduced: Option<Vec<SocketAddr>>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreRoute {
    /// Requests to the Core go here. They are answered `core_offline` while it is not reached.
    pub base: String,
    pub reached: Option<Reached>,
    /// This device, for the backend to introduce to the Core. Set when the Core was not reached
    /// and no introduction was given.
    pub dialer: Option<Dialer>,
}

/// How a Core is reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Reached {
    /// On this computer or its network.
    Local,
    /// Across the internet, through both sides' routers.
    Internet,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewCoreSession {
    pub core_id: String,
    pub account: String,
    pub fingerprint: String,
    pub ticket: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreSessionKey {
    pub core_id: String,
    pub account: String,
}
