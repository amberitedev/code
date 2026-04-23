//! State management for Theseus
//! Simplified from the full Theseus State system

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::OnceCell;
use sqlx::SqlitePool;

use crate::theseus::profile::DirectoryInfo;
use crate::theseus::error::{TheseusError, TheseusResult};

static THESEUS_STATE: OnceCell<Arc<State>> = OnceCell::const_new();

/// Global Theseus state
pub struct State {
    pub directories: DirectoryInfo,
    pub pool: SqlitePool,
    pub http_client: reqwest::Client,
}

impl State {
    /// Initialize the global state
    pub async fn init(
        base_dir: PathBuf,
        pool: SqlitePool,
    ) -> TheseusResult<()> {
        let state = Self::initialize_state(base_dir, pool).await?;
        THESEUS_STATE
            .set(state)
            .map_err(|_| TheseusError::Other("State already initialized".to_string()))?;
        Ok(())
    }
    
    /// Get the current state
    pub async fn get() -> TheseusResult<Arc<State>> {
        if !THESEUS_STATE.initialized() {
            return Err(TheseusError::Other(
                "State not initialized".to_string()
            ));
        }
        
        Ok(Arc::clone(
            THESEUS_STATE.get().expect("State not initialized")
        ))
    }
    
    /// Check if state is initialized
    pub fn initialized() -> bool {
        THESEUS_STATE.initialized()
    }
    
    async fn initialize_state(
        base_dir: PathBuf,
        pool: SqlitePool,
    ) -> TheseusResult<Arc<Self>> {
        let directories = DirectoryInfo::new(base_dir);
        
        // Create necessary directories
        tokio::fs::create_dir_all(directories.profiles_dir()).await?;
        tokio::fs::create_dir_all(directories.libraries_dir()).await?;
        tokio::fs::create_dir_all(directories.assets_dir()).await?;
        tokio::fs::create_dir_all(directories.versions_dir()).await?;
        
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;
        
        Ok(Arc::new(Self {
            directories,
            pool,
            http_client,
        }))
    }
}

/// Fetch JSON from a URL
pub async fn fetch_json<T: serde::de::DeserializeOwned>(
    url: &str,
) -> TheseusResult<T> {
    let state = State::get().await?;
    let response = state.http_client.get(url).send().await?;
    let json = response.json().await?;
    Ok(json)
}

/// Fetch bytes from a URL
pub async fn fetch_bytes(url: &str) -> TheseusResult<bytes::Bytes> {
    let state = State::get().await?;
    let response = state.http_client.get(url).send().await?;
    let bytes = response.bytes().await?;
    Ok(bytes)
}
