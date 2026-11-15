//! Routes the local dev backend serves to the dev runner and dev Apps. Elsewhere they answer 404.
//! A caller sends the runner's secret in `X-Local-Dev-Secret`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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

/// Body of [`Sql`]: one statement and the values for its `?` marks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SqlStatement {
    pub sql: String,
    #[cfg_attr(feature = "ts", ts(type = "Array<string | number | null>"))]
    pub params: Vec<Value>,
}

/// What a statement read, and how many rows it changed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SqlAnswer {
    #[cfg_attr(
        feature = "ts",
        ts(type = "Array<Record<string, string | number | null>>")
    )]
    pub rows: Vec<Map<String, Value>>,
    pub changes: u32,
}

/// Signs in to a scenario account without a password. Accounts made any other way are refused.
pub struct Session;

/// The last 100 emails, newest first.
pub struct Outbox;

/// Runs one statement on the backend's database. A dev fixture reads rows with it, and changes
/// one the backend wrote into a state the backend never reaches by itself.
pub struct Sql;

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

impl Endpoint for Sql {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/_dev/sql";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<SqlStatement>;
    type Response = Json<SqlAnswer>;
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
        SqlStatement::decl(&cfg),
        SqlAnswer::decl(&cfg),
    ]
    .map(|declaration| format!("\nexport {declaration}\n"))
    .concat();
    format!(
        "{declarations}\n/** The local dev backend's routes. */\nexport const dev = {{\n\tSession: {},\n\tOutbox: {},\n\tSql: {},\n}}\n",
        endpoint::<Session>(),
        endpoint::<Outbox>(),
        endpoint::<Sql>(),
    )
}
