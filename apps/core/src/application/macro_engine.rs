//! Macro engine - Executes user-defined macros and scripts.

use std::sync::Arc;
use thiserror::Error;
use crate::domain::ports::ScriptRuntime;

/// Macro engine error.
#[derive(Error, Debug)]
pub enum MacroEngineError {
    #[error("Execution failed: {0}")]
    Execution(String),
}

/// Macro engine for user scripts.
pub struct MacroEngine {
    script_runtime: Arc<dyn ScriptRuntime>,
}

impl MacroEngine {
    pub fn new(script_runtime: Arc<dyn ScriptRuntime>) -> Self {
        MacroEngine { script_runtime }
    }

    pub async fn execute(&self, code: &str) -> Result<(), MacroEngineError> {
        self.script_runtime.execute(code)
            .await
            .map_err(|e| MacroEngineError::Execution(e.to_string()))?;
        Ok(())
    }
}
