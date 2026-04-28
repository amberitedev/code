use async_trait::async_trait;

use crate::domain::modpack::ModpackManifest;
use super::instance_store::StoreError;

#[async_trait]
pub trait ModpackStore: Send + Sync + 'static {
    async fn save(&self, manifest: &ModpackManifest) -> Result<(), StoreError>;
    async fn get_for_instance(
        &self,
        instance_id: &str,
    ) -> Result<Option<ModpackManifest>, StoreError>;
    async fn delete_for_instance(&self, instance_id: &str) -> Result<(), StoreError>;
}
