//! Who may use a Core: the owner's sessions, the activity log and pairing.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A user as their last ticket named them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreUser {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewSession {
    /// A ticket the backend signed for this Core, under five minutes old and never used.
    pub ticket: String,
    /// Up to 64 characters.
    pub device_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SignedIn {
    /// 256 random bits, sent as the bearer. The Core stores only its hash, and the App keeps it in
    /// the OS keychain, out of the WebView.
    pub token: String,
    pub session: CoreSession,
}

/// One device signed in to a Core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreSession {
    /// Public id for listing and revoking. Not the token.
    pub id: String,
    pub user: CoreUser,
    pub device_name: String,
    pub created_at: DateTime<Utc>,
    /// Written at most once an hour.
    pub last_used_at: DateTime<Utc>,
    /// The session ends at this time unless it is used before then; it never lasts past a year
    /// from `created_at`.
    pub expires_at: DateTime<Utc>,
    /// The session making this request.
    pub current: bool,
}

/// Which entries `ListActivity` lists, as Modrinth Hosting's action log takes them
/// (`Archon.Actions.v1.ListActionLogOptions`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ActivityQuery {
    /// Only this server's entries.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub server_id: Option<Uuid>,
    /// An [`ActionLogFilter`] as JSON.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub filter: Option<String>,
    /// At most 100. 50 when left out.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub limit: Option<u32>,
    /// Entries to skip, for the next page.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub offset: Option<u32>,
    /// Newest first when left out.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub order: Option<SortOrder>,
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub min_datetime: Option<DateTime<Utc>>,
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub max_datetime: Option<DateTime<Utc>>,
}

/// Entries must match each list given.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ActionLogFilter {
    /// Ids of the users who acted.
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub users: Option<Vec<String>>,
    /// [`Action`] names, as its `action` field holds them.
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub actions: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ActionLogResponse {
    pub data: Vec<ActionEntry>,
    /// Pass as `offset` for the next page. Null on the last page.
    pub next_offset: Option<u32>,
    /// Every user `data` names, by id.
    pub users: BTreeMap<String, UserResp>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct UserResp {
    pub username: String,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ActionEntry {
    pub actor: ActionUser,
    pub action: Action,
    /// Null for actions on the Core itself.
    pub server_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Who acted: a signed-in user, or the Core's local CLI, which is the owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionUser {
    User { user_id: String },
    Cli,
}

/// What was done, named as Modrinth Hosting names the same action. The Core stores the name and
/// the metadata apart, so a renamed variant or field hides the entries already stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "action", content = "metadata", rename_all = "snake_case")]
pub enum Action {
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
    ConsoleCommandExecuted {
        command: String,
    },
    SettingsChanged,
    ContentChanged,
    BackupCreated {
        id: Uuid,
    },
    BackupRestored {
        id: Uuid,
    },
    BackupDeleted {
        id: Uuid,
    },
    BackupRenamed {
        id: Uuid,
    },
    BackupDownloaded {
        id: Uuid,
    },
    WorldDownloaded,
    EulaAccepted,
    CoreUpdated {
        version: String,
    },
    /// The owner removed this Core from their account.
    CoreRemoved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PairingStatus {
    pub paired: bool,
    /// The owner's name while paired.
    pub owner: Option<String>,
    /// The code to enter in the App, like `MZUX-D576`. Null once paired, and while the Core is
    /// still getting one from Amberite.
    pub code: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
pub(super) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![
        CoreUser::decl(&cfg),
        NewSession::decl(&cfg),
        SignedIn::decl(&cfg),
        CoreSession::decl(&cfg),
        ActivityQuery::decl(&cfg),
        ActionLogFilter::decl(&cfg),
        SortOrder::decl(&cfg),
        ActionLogResponse::decl(&cfg),
        UserResp::decl(&cfg),
        ActionEntry::decl(&cfg),
        ActionUser::decl(&cfg),
        Action::decl(&cfg),
        PairingStatus::decl(&cfg),
    ]
}
