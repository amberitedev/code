//! Routes the local dev backend serves to the dev runner and dev Apps. Elsewhere they answer 404.
//! A caller sends the runner's secret in `X-Local-Dev-Secret`.

use serde::{Deserialize, Serialize};

use super::accounts::AuthSession;
use crate::{Auth, Empty, Endpoint, Json, Method};

/// Body of [`Session`]: a scenario account, created on first use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScenarioLogin {
    pub username: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum EmailKind {
    VerifyEmail,
    ResetPassword,
}

/// An email the backend would have sent. `flow` is what its link carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OutboxEmail {
    pub id: String,
    pub recipient: String,
    pub kind: EmailKind,
    pub flow: String,
    pub created: String,
}

/// Signs in to a scenario account without a password. Accounts made any other way are refused.
pub struct Session;

/// The last 100 emails, newest first.
pub struct Outbox;

impl Endpoint for Session {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/_dev/session";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<ScenarioLogin>;
    type Response = Json<AuthSession>;
}

impl Endpoint for Outbox {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/_dev/outbox";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<Vec<OutboxEmail>>;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    use crate::ts::endpoint;

    let cfg = crate::ts::config();
    let declarations = [
        ScenarioLogin::decl(&cfg),
        EmailKind::decl(&cfg),
        OutboxEmail::decl(&cfg),
    ]
    .map(|declaration| format!("\nexport {declaration}\n"))
    .concat();
    format!(
        "{declarations}\n/** The local dev backend's routes. */\nexport const dev = {{\n\tSession: {},\n\tOutbox: {},\n}}\n",
        endpoint::<Session>(),
        endpoint::<Outbox>(),
    )
}
