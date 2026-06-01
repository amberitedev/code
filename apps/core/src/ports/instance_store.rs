use async_trait::async_trait;

use crate::domain::instance::{
    InstanceId, InstanceInstallStatus, InstanceRecord, InstanceStatus, ModLoader,
};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("parse: {0}")]
    Parse(String),
}

#[async_trait]
pub trait InstanceStore: Send + Sync + 'static {
    async fn create(&self, record: &InstanceRecord) -> Result<(), StoreError>;
    async fn get(&self, id: &InstanceId) -> Result<InstanceRecord, StoreError>;
    async fn list(&self) -> Result<Vec<InstanceRecord>, StoreError>;
    async fn update_status(
        &self,
        id: &InstanceId,
        status: InstanceStatus,
    ) -> Result<(), StoreError>;
    async fn update_port(
        &self,
        id: &InstanceId,
        port: u16,
    ) -> Result<(), StoreError>;
    async fn update_name(
        &self,
        id: &InstanceId,
        name: &str,
    ) -> Result<(), StoreError>;
    async fn update_java_version(
        &self,
        id: &InstanceId,
        java_version: Option<i64>,
    ) -> Result<(), StoreError>;
    /// Update JVM heap settings (megabytes). Applied on the next start.
    async fn update_memory(
        &self,
        id: &InstanceId,
        min_mb: u32,
        max_mb: u32,
    ) -> Result<(), StoreError>;
    /// Update custom launch tuning. `None` clears the override; applied on next start.
    async fn update_startup(
        &self,
        id: &InstanceId,
        jvm_args: Option<&str>,
        server_args: Option<&str>,
    ) -> Result<(), StoreError>;
    /// Update the game version, loader, and loader version of an instance.
    /// Used by `change_version` when migrating an instance to a different build.
    async fn update_version(
        &self,
        id: &InstanceId,
        game_version: &str,
        loader: &ModLoader,
        loader_version: Option<&str>,
    ) -> Result<(), StoreError>;
    async fn update_install_status(
        &self,
        id: &InstanceId,
        install_status: InstanceInstallStatus,
    ) -> Result<(), StoreError>;
    /// Bind (or unbind, with `None`) an instance to a shared installation.
    async fn update_installation_id(
        &self,
        id: &InstanceId,
        installation_id: Option<&str>,
    ) -> Result<(), StoreError>;
    async fn delete(&self, id: &InstanceId) -> Result<(), StoreError>;
    async fn list_by_status(
        &self,
        status: InstanceStatus,
    ) -> Result<Vec<InstanceRecord>, StoreError>;
    /// List all instances bound to a given shared installation.
    async fn list_by_installation(
        &self,
        installation_id: &str,
    ) -> Result<Vec<InstanceRecord>, StoreError>;
    /// Reset any instances stuck in transient states (`starting`/`stopping`) to `offline`.
    /// Called once on startup to recover from unclean shutdown.
    async fn reset_transient_statuses(&self) -> Result<u64, StoreError>;
    /// Add `seconds` to the accumulated `total_uptime_seconds` for the given instance.
    async fn add_uptime(&self, id: &InstanceId, seconds: u64) -> Result<(), StoreError>;
}
