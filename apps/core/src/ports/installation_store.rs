use async_trait::async_trait;

use crate::domain::server_installation::{
    InstallationId, InstallationStatus, ServerInstallationRecord,
};
use crate::ports::instance_store::StoreError;

/// Port for persisting and querying shared server installations.
#[async_trait]
pub trait InstallationStore: Send + Sync + 'static {
    /// Insert a new installation record.
    async fn create(
        &self,
        record: &ServerInstallationRecord,
    ) -> Result<(), StoreError>;
    /// Fetch a single installation by id, or `None` if it does not exist.
    async fn get(
        &self,
        id: &InstallationId,
    ) -> Result<Option<ServerInstallationRecord>, StoreError>;
    /// List all known installations.
    async fn list(&self) -> Result<Vec<ServerInstallationRecord>, StoreError>;
    /// List all installations currently in a given status (e.g. stuck `Installing`).
    async fn list_by_status(
        &self,
        status: InstallationStatus,
    ) -> Result<Vec<ServerInstallationRecord>, StoreError>;
    /// Update an installation's status and optional error message.
    async fn update_status(
        &self,
        id: &InstallationId,
        status: InstallationStatus,
        error: Option<&str>,
    ) -> Result<(), StoreError>;
    /// Delete an installation record.
    async fn delete(&self, id: &InstallationId) -> Result<(), StoreError>;
}
