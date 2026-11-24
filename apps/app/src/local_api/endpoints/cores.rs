//! Paired Cores: the proxy address that reaches each, and this device's session there. See
//! `crate::cores`.

use contracts::{Empty, Json, core::CoreSession};
use serde::{Deserialize, Serialize};

contracts::endpoints! {
    group pairedCores;

    /// Returns the base URL on the App's Core proxy that reaches the Core, trusting only the
    /// certificate with `fingerprint` and signing with `account`'s session there, when it has one.
    ConnectCore: Post "/app/cores/connect", None, (), (), Json<CoreTarget> => Json<String>;
    /// Trades a ticket for a session on the Core, named after this computer, and keeps the token for
    /// `account`. Returns the session, never the token.
    SignInToCore: Post "/app/cores/sessions", None, (), (), Json<NewCoreSession> => Json<CoreSession>;
    /// Ends `account`'s session on the Core, when it answers, and forgets the token either way.
    SignOutOfCore: Delete "/app/cores/sessions", None, (), CoreSessionKey, Empty => Empty;
    /// Whether `account` holds a session for the Core. It may have expired there.
    CheckCoreSession: Get "/app/cores/sessions", None, (), CoreSessionKey, Empty => Json<bool>;
}

/// A paired Core as `account` reaches it: its `https://host:port` address and the SHA-256 of its
/// certificate.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreTarget {
    pub core_id: String,
    pub account: String,
    pub address: String,
    pub fingerprint: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewCoreSession {
    pub core_id: String,
    pub account: String,
    pub address: String,
    pub fingerprint: String,
    pub ticket: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreSessionKey {
    pub core_id: String,
    pub account: String,
}
