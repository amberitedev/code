//! Amberite Backend - Core logic for Amberite-specific features
//!
//! Provides Amberite-specific functionality: Supabase auth, OS keychain
//! access, core process management, pairing, and more.

pub mod auth;
pub mod console_stream;
pub mod core_launcher;
pub mod error;
pub mod pairing;
pub mod progress_stream;
pub mod settings;
pub mod tunnel;

pub use error::{AmberiteError, Result};

use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

/// Shared Amberite runtime state, held as Tauri managed state
pub struct AmberiteState {
    /// Shared HTTP client for all outbound requests
    pub http_client: Client,
    /// App settings (loaded at startup, saved on change)
    pub settings: Arc<RwLock<settings::AppSettings>>,
    /// Handle to a running Amberite Core child process (if launched locally)
    pub core_process: Arc<Mutex<Option<core_launcher::CoreProcess>>>,
    /// Active console-streaming tasks keyed by instance ID
    pub console_tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl AmberiteState {
    /// Create a new AmberiteState, loading settings from disk
    pub async fn new() -> Result<Self> {
        let http_client = Client::builder()
            .use_rustls_tls()
            .build()
            .map_err(|e| AmberiteError::Http(e.to_string()))?;

        let settings = settings::AppSettings::load().await?;

        Ok(Self {
            http_client,
            settings: Arc::new(RwLock::new(settings)),
            core_process: Arc::new(Mutex::new(None)),
            console_tasks: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}
