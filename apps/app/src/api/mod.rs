use engine::error_code;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

pub mod instance;
pub mod logs;
pub mod shortcuts;
pub mod utils;

pub(crate) mod oauth_utils;

pub type Result<T> = std::result::Result<T, CommandError>;

#[derive(Debug)]
pub struct CommandError(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for CommandError {
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

impl Serialize for CommandError {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        tracing::warn!("{:?}", self.0);

        let mut state = serializer.serialize_struct("CommandError", 2)?;
        state.serialize_field("message", &format!("{:#}", self.0))?;
        state.serialize_field("code", error_code(&self.0))?;
        state.end()
    }
}
