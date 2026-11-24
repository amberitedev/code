//! Who may use a Core: the owner's sessions, the activity log and pairing.

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
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds, written at most once an hour.
    pub last_used_at: i64,
    /// Unix seconds. The session ends at this time unless it is used before then; it never
    /// lasts past a year from `created_at`.
    pub expires_at: i64,
    /// The session making this request.
    pub current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ActivityQuery {
    /// Only this server's entries.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub server_id: Option<Uuid>,
    /// Entries with an id below this, for the next page.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub before: Option<i64>,
    /// At most 100. 50 when left out.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ActivityPage {
    pub entries: Vec<ActivityEntry>,
    /// Pass as `before` for the next page. Null on the last page.
    pub next_before: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ActivityEntry {
    pub id: i64,
    /// Unix seconds.
    pub at: i64,
    /// Null for the local CLI, which is the owner.
    pub user: Option<CoreUser>,
    /// Null for actions on the Core itself.
    pub server_id: Option<Uuid>,
    pub action: Action,
}

/// What was done. The Core stores the tag and the fields apart, so a renamed variant or field
/// hides the entries already stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
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
    BackupRenamed {
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
    /// The owner removed this Core from their account.
    CoreRemoved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PairingStatus {
    pub paired: bool,
    /// The code to enter in the App, like `MZUX-D576`. Null once paired, and while the Core is
    /// still getting one from Amberite.
    pub code: Option<String>,
    /// Unix seconds.
    pub expires_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewDownloadLink {
    /// The exact path and query the link will GET, such as
    /// `/instances/{id}/files/content?path=server.properties`.
    pub path: String,
}

/// Sent as `?link=` on a GET of the path it was made for: once, within five minutes, and only
/// while its maker may still make it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct DownloadLink {
    pub token: String,
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
        ActivityPage::decl(&cfg),
        ActivityEntry::decl(&cfg),
        Action::decl(&cfg),
        PairingStatus::decl(&cfg),
        NewDownloadLink::decl(&cfg),
        DownloadLink::decl(&cfg),
    ]
}
