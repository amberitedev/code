//! The activity log: access changes, sessions, sensitive reads, and server actions, with who did
//! them. Owner only.
//!
//! - `GET /activity`: newest first. [`ActivityQuery`] as query parameters, [`ActivityPage`] out.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{
    AppState,
    access::{Role, User},
    error::ApiError,
};
use crate::access::{Caller, activity};

/// `GET /activity`. Owner only.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<ActivityPage>, ApiError> {
    caller.require_owner()?;
    Ok(Json(activity::page(&state.db, &query).await?))
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct ActivityQuery {
    /// Only this server's entries.
    #[ts(optional)]
    pub server_id: Option<Uuid>,
    /// Entries with an id below this, for the next page.
    #[ts(optional)]
    pub before: Option<i64>,
    /// At most 100. 50 when left out.
    #[ts(optional)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct ActivityPage {
    pub entries: Vec<ActivityEntry>,
    /// Pass as `before` for the next page. Null on the last page.
    pub next_before: Option<i64>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct ActivityEntry {
    pub id: i64,
    /// Unix seconds.
    pub at: i64,
    /// Null for the local CLI, which is the owner.
    pub user: Option<User>,
    /// Null for actions on the Core itself.
    pub server_id: Option<Uuid>,
    pub action: Action,
}

/// What was done. Stored as the tag in `activity.action` and the fields in `activity.detail`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Invited {
        user_id: String,
        role: Role,
    },
    RoleChanged {
        user_id: String,
        role: Role,
    },
    Removed {
        user_id: String,
    },
    SignedIn {
        session_id: String,
        device_name: String,
    },
    SessionRevoked {
        session_id: String,
    },
    ServerCreated,
    ServerDeleted,
    ServerStarted,
    ServerStopped,
    ServerRestarted,
    ServerKilled,
    ConsoleCommand {
        command: String,
    },
    SettingsChanged,
    ContentChanged,
    BackupCreated {
        backup_id: Uuid,
    },
    BackupRestored {
        backup_id: Uuid,
    },
    BackupDeleted {
        backup_id: Uuid,
    },
    BackupDownloaded {
        backup_id: Uuid,
    },
    WorldDownloaded,
    EulaAccepted,
    CoreUpdated {
        version: String,
    },
}
