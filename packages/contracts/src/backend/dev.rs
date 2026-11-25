//! Routes the local dev backend serves to the dev runner and dev Apps. Elsewhere they answer 404.
//! A caller sends the runner's secret in `X-Local-Dev-Secret`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::accounts::AuthSession;
use crate::{Empty, Json};

/// Body of [`Session`]: a scenario account, created on first use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ScenarioLogin {
    pub username: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum EmailKind {
    VerifyEmail,
    ResetPassword,
}

/// An email the backend would have sent. `flow` is what its link carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct OutboxEmail {
    pub id: String,
    pub recipient: String,
    pub kind: EmailKind,
    pub flow: String,
    pub created: DateTime<Utc>,
}

/// Body of [`Sql`]: one statement and the values for its `?` marks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SqlStatement {
    pub sql: String,
    #[cfg_attr(
        feature = "export-ts",
        ts(type = "Array<string | number | null>")
    )]
    pub params: Vec<Value>,
}

/// What a statement read, and how many rows it changed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SqlAnswer {
    #[cfg_attr(
        feature = "export-ts",
        ts(type = "Array<Record<string, string | number | null>>")
    )]
    pub rows: Vec<Map<String, Value>>,
    pub changes: u32,
}

crate::endpoints! {
    /// The local dev backend's routes.
    group dev;

    /// Signs in to a scenario account without a password. Accounts made any other way are refused.
    Session: Post "/_dev/session", None, (), (), Json<ScenarioLogin> => Json<AuthSession>;
    /// The last 100 emails, newest first.
    Outbox: Get "/_dev/outbox", None, (), (), Empty => Json<Vec<OutboxEmail>>;
    /// Runs one statement on the backend's database. A dev fixture reads rows with it, and changes
    /// one the backend wrote into a state the backend never reaches by itself.
    Sql: Post "/_dev/sql", None, (), (), Json<SqlStatement> => Json<SqlAnswer>;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

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
    declarations + &ts_module().ts(false)
}
