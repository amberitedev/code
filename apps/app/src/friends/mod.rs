pub mod api;
pub mod routes;
pub mod state;

use serde::{Deserialize, Serialize};

/// A change the friends socket reports, sent to the UI as `AppEvent::Friend`.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
#[serde(tag = "event")]
#[cfg_attr(feature = "export-ts", ts(tag = "event", rename_all = "snake_case"))]
pub enum FriendPayload {
    FriendRequest { from: String },
    UserOffline { id: String },
    StatusUpdate { user_status: FriendStatusPayload },
    StatusSync,
    FriendRequestRejected { from: String },
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct FriendStatusPayload {
    pub user_id: String,
    pub profile_name: Option<String>,
    pub last_update: String,
}
