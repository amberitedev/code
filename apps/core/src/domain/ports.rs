//! Domain ports - Dependency inversion interfaces.
//! Application layer uses these to talk to outside world.

use async_trait::async_trait;
use crate::domain::instances::{GameInstance, InstanceId, Stopped};
use crate::domain::auth::{UserId, User};
use crate::domain::config::SettingManifest;
use crate::domain::instances::DomainError;

/// Repository port for instances.
#[async_trait]
pub trait InstanceRepository: Send + Sync {
    async fn get_instance(&self, id: InstanceId) -> Result<GameInstance<Stopped>, DomainError>;
    async fn save_instance(&self, instance: &GameInstance<Stopped>) -> Result<(), DomainError>;
    async fn list_instances(&self) -> Result<Vec<GameInstance<Stopped>>, DomainError>;
    async fn delete_instance(&self, id: InstanceId) -> Result<(), DomainError>;
}

/// Repository port for users.
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn get_user(&self, id: UserId) -> Result<User, DomainError>;
    async fn get_user_by_username(&self, username: &str) -> Result<User, DomainError>;
    async fn create_user(&self, user: &User) -> Result<(), DomainError>;
    async fn update_user(&self, user: &User) -> Result<(), DomainError>;
    async fn list_users(&self) -> Result<Vec<User>, DomainError>;
}

/// Process manager port.
#[async_trait]
pub trait OSProcessManager: Send + Sync {
    async fn spawn(&self, cmd: &str) -> Result<Box<dyn crate::domain::instances::ProcessHandle>, DomainError>;
    async fn kill(&self, pid: i32) -> Result<(), DomainError>;
}

/// Script runtime port.
#[async_trait]
pub trait ScriptRuntime: Send + Sync {
    async fn execute(&self, code: &str) -> Result<(), DomainError>;
}

/// Config manager port.
#[async_trait]
pub trait ConfigManager: Send + Sync {
    async fn read_properties(&self, path: &std::path::Path) -> Result<Vec<SettingManifest>, DomainError>;
    async fn write_properties(&self, path: &std::path::Path, settings: &[SettingManifest]) -> Result<(), DomainError>;
}
