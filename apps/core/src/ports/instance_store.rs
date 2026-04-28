use async_trait::async_trait;

use crate::domain::instance::{InstanceId, InstanceRecord, InstanceStatus};

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
    async fn delete(&self, id: &InstanceId) -> Result<(), StoreError>;
    async fn list_by_status(
        &self,
        status: InstanceStatus,
    ) -> Result<Vec<InstanceRecord>, StoreError>;
}
