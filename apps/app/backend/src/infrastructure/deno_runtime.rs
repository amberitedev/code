use std::thread;
use std::sync::mpsc;
use thiserror::Error;
use crate::domain::ports::ScriptRuntime;
use crate::domain::instances::DomainError;

/// Deno runtime error.
#[derive(Error, Debug)]
pub enum DenoRuntimeError {
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Channel send failed: {0}")]
    ChannelSend(String),
}

/// Message to send to the Deno runtime thread.
enum DenoMessage {
    Execute {
        code: String,
        tx: mpsc::Sender<Result<(), DenoRuntimeError>>,
    },
}

/// Deno runtime for JavaScript macros.
/// Runs on a dedicated thread to isolate the !Send JsRuntime.
pub struct DenoRuntime {
    tx: mpsc::Sender<DenoMessage>,
}

impl DenoRuntime {
    pub fn new() -> Result<Self, DenoRuntimeError> {
        let (tx, rx) = mpsc::channel::<DenoMessage>();

        thread::spawn(move || {
            let mut runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions::default());

            while let Ok(msg) = rx.recv() {
                match msg {
                    DenoMessage::Execute { code, tx } => {
                        let _ = tx.send(
                            runtime.execute_script("<macro>", code)
                                .map(|_| ())
                                .map_err(|e| DenoRuntimeError::ExecutionFailed(e.to_string()))
                        );
                    }
                }
            }
        });

        Ok(DenoRuntime { tx })
    }
}

#[async_trait::async_trait]
impl ScriptRuntime for DenoRuntime {
    async fn execute(&self, code: &str) -> Result<(), DomainError> {
        let (tx, rx) = mpsc::channel();
        let msg = DenoMessage::Execute {
            code: code.to_string(),
            tx,
        };

        self.tx.send(msg).map_err(|e| {
            DenoRuntimeError::ChannelSend(e.to_string())
        })?;

        match rx.recv() {
            Ok(result) => result.map_err(|e| DomainError::Database(e.to_string())),
            Err(e) => Err(DomainError::Database(e.to_string())),
        }
    }
}

/// Mock Deno runtime for testing.
pub struct MockDenoRuntime;

impl MockDenoRuntime {
    pub fn new() -> Self {
        MockDenoRuntime
    }
}

#[async_trait::async_trait]
impl ScriptRuntime for MockDenoRuntime {
    async fn execute(&self, _code: &str) -> Result<(), DomainError> {
        Ok(())
    }
}