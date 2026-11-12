//! Who may use a Core: sessions, per-server roles, the activity log and pairing.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A role on one server. The owner has every role on every server and is never a member.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[cfg_attr(
    feature = "sqlx",
    derive(sqlx::Type),
    sqlx(type_name = "TEXT", rename_all = "snake_case")
)]
#[serde(rename_all = "snake_case")]
pub enum CoreRole {
    /// Sees status, players, content, and backups. No console: it carries chat and player IPs.
    Viewer,
    /// Everything a viewer can, plus running, console, files, settings, and backups. Runs code on
    /// the host machine.
    Editor,
}

/// A user as their last ticket named them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CoreUser {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Member {
    pub user: CoreUser,
    pub role: CoreRole,
    /// Unix seconds.
    pub added_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerAccess {
    /// Null until the Core is paired.
    pub owner: Option<CoreUser>,
    pub members: Vec<Member>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewMember {
    /// Shown until the invited user signs in and their own ticket names them.
    pub user: CoreUser,
    pub role: CoreRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MemberUpdate {
    pub role: CoreRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MyAccess {
    pub user: CoreUser,
    /// The owner of this Core, with every role on every server.
    pub owner: bool,
    /// Empty for the owner.
    pub servers: Vec<ServerRole>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerRole {
    pub server_id: Uuid,
    pub role: CoreRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewSession {
    /// A ticket the backend signed for this Core, under five minutes old and never used.
    pub ticket: String,
    /// Up to 64 characters.
    pub device_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SignedIn {
    /// 256 random bits, sent as the bearer. The Core stores only its hash, and the App keeps it in
    /// the OS keychain, out of the WebView.
    pub token: String,
    pub session: CoreSession,
}

/// One device signed in to a Core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ActivityQuery {
    /// Only this server's entries.
    #[cfg_attr(feature = "ts", ts(optional))]
    pub server_id: Option<Uuid>,
    /// Entries with an id below this, for the next page.
    #[cfg_attr(feature = "ts", ts(optional))]
    pub before: Option<i64>,
    /// At most 100. 50 when left out.
    #[cfg_attr(feature = "ts", ts(optional))]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ActivityPage {
    pub entries: Vec<ActivityEntry>,
    /// Pass as `before` for the next page. Null on the last page.
    pub next_before: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Invited {
        user_id: String,
        role: CoreRole,
    },
    RoleChanged {
        user_id: String,
        role: CoreRole,
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PairingStatus {
    pub paired: bool,
    /// The code to enter in the App, like `MZUX-D576`. Null once paired, and while the Core is
    /// still getting one from Amberite.
    pub code: Option<String>,
    /// Unix seconds.
    pub expires_at: Option<i64>,
}

#[cfg(test)]
pub(super) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![
        CoreRole::decl(&cfg),
        CoreUser::decl(&cfg),
        Member::decl(&cfg),
        ServerAccess::decl(&cfg),
        NewMember::decl(&cfg),
        MemberUpdate::decl(&cfg),
        MyAccess::decl(&cfg),
        ServerRole::decl(&cfg),
        NewSession::decl(&cfg),
        SignedIn::decl(&cfg),
        CoreSession::decl(&cfg),
        ActivityQuery::decl(&cfg),
        ActivityPage::decl(&cfg),
        ActivityEntry::decl(&cfg),
        Action::decl(&cfg),
        PairingStatus::decl(&cfg),
    ]
}
