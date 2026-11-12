//! Routes the backend serves.

use crate::{Auth, Empty, Endpoint, Method};

pub mod accounts;
pub mod cores;
pub mod dev;
pub mod social;

/// Answers once the backend reaches its database.
pub struct CheckHealth;

impl Endpoint for CheckHealth {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/health";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Empty;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    format!(
        "\n/** The backend's routes outside accounts, cores, dev and social, which have their own consts. */\nexport const backend = {{\n\tCheckHealth: {},\n}}\n",
        crate::ts::endpoint::<CheckHealth>()
    )
}
