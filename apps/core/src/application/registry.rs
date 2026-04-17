//! Service registry - Narrow dependency injection instead of AppState God-object.

use crate::application::auth_service::AuthService;
use crate::application::instance_service::InstanceService;
use crate::application::macro_engine::MacroEngine;
use std::sync::Arc;

/// ServiceRegistry provides narrow dependency injection.
#[derive(Clone)]
pub struct ServiceRegistry {
    pub auth_service: Arc<AuthService>,
    pub instance_service: Arc<InstanceService>,
    pub macro_engine: Arc<MacroEngine>,
}

impl ServiceRegistry {
    pub fn new(
        auth_service: Arc<AuthService>,
        instance_service: Arc<InstanceService>,
        macro_engine: Arc<MacroEngine>,
    ) -> Self {
        ServiceRegistry {
            auth_service,
            instance_service,
            macro_engine,
        }
    }
}
