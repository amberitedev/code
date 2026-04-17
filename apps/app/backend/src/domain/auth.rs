//! Domain auth types - UserId, User, Role, UserPermission.
//! Uses Newtype pattern for type safety.

use derive_more::Display;
use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

/// Newtype wrapper for User IDs.
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, Display, Debug, serde::Serialize, serde::Deserialize,
)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn new(id: Uuid) -> Self {
        UserId(id)
    }
}

/// User represents a system user.
pub struct User {
    pub id: UserId,
    pub username: String,
    pub hashed_password: String,
    pub role: Role,
}

/// Role defines permission level.
pub enum Role {
    Admin,
    Standard(UserPermission),
}

/// InstanceId forward declaration.
use crate::domain::instances::InstanceId;

/// UserPermission defines what a standard user can do.
pub struct UserPermission {
    pub can_start_instance: HashSet<InstanceId>,
    pub can_stop_instance: HashSet<InstanceId>,
    pub can_edit_config: HashSet<InstanceId>,
}

impl Default for UserPermission {
    fn default() -> Self {
        UserPermission {
            can_start_instance: HashSet::new(),
            can_stop_instance: HashSet::new(),
            can_edit_config: HashSet::new(),
        }
    }
}

/// Authentication token payload.
#[derive(Serialize)]
pub struct AuthTokenPayload {
    pub user_id: UserId,
    pub expires_at: i64,
}

impl AuthTokenPayload {
    pub fn new(user_id: UserId, expires_at: i64) -> Self {
        AuthTokenPayload {
            user_id,
            expires_at,
        }
    }
}
