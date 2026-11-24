//! The backend's Core routes. A Core signs in with a code its owner approves in the App, then
//! calls with its own key (`Bearer amc_...`): it reports its state on start and on change, and its
//! storage after uploads, pulls and hourly. A key the backend no longer knows gets 410
//! `core_removed`, and the Core un-pairs. The App lists the account's Cores, approves codes and
//! gets the tickets it signs in to a Core with.
//!
//! A Core reaches shares with its key on `/core/shares`, acting for its owner: it writes the share
//! of a server that follows no instance, and reads any share its owner reads, so a server can
//! follow one.

use serde::{Deserialize, Serialize};

use super::sharing::{
    CreatedId, InstanceVersion, InstanceVersionId, NewInstanceVersion,
    SharedInstanceId, SharedInstanceUpdate,
};
use crate::{Empty, Json, Upgrade, tickets::PublicKeys};

/// Body of [`RequestSignInCode`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewSignInCode {
    pub core_id: String,
    /// The machine's name, shown to the owner before they approve.
    pub name: String,
    /// Where the App reaches the Core, `https://ip:port`.
    pub address: String,
    /// SHA-256 of the Core's certificate, lowercase hex.
    pub cert_fingerprint: Option<String>,
}

/// A code for the owner to approve, and the key the Core uses once they do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SignInCode {
    pub code: String,
    pub key: String,
    /// Seconds.
    pub expires_in: i64,
    /// Seconds.
    pub poll_interval: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SignInStatus {
    Pending,
    Approved { account: CoreOwner },
}

/// The account that approved a Core's code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreOwner {
    pub id: String,
    pub username: String,
}

/// Body of [`ReportState`]: everything the backend keeps about a Core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct StateReport {
    pub version: String,
    /// Where the App reaches this Core on its own network, `https://ip:port`.
    pub address: String,
    /// Where the App reaches it from outside, once known.
    pub public_address: Option<String>,
    /// SHA-256 of the TLS certificate, lowercase hex. The App pins it.
    pub cert_fingerprint: String,
}

/// Body of [`ReportStorage`]. Bytes throughout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct StorageReport {
    /// This Core offers storage to other people's shares. Off, it holds only its owner's.
    pub enabled: bool,
    /// The Core is installed as a service.
    pub service: bool,
    pub cap: u64,
    pub used: u64,
    /// Free on the store's disk.
    pub free: u64,
    /// What it may serve a day, and has served today (UTC).
    pub egress_cap: u64,
    pub egress_today: u64,
    /// Every version the store holds whole.
    pub versions: Vec<HeldVersion>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct HeldVersion {
    /// The version's hash.
    pub root: String,
    /// The bytes of its objects.
    pub bytes: u64,
}

/// What the backend answers a [`StorageReport`] with.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct StorageAnswer {
    /// Versions to stop holding.
    #[serde(default)]
    pub drop: Vec<String>,
    /// Versions to copy from another Core.
    #[serde(default)]
    pub pull: Vec<PullOrder>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PullOrder {
    pub root: String,
    /// A pull ticket naming the source Core.
    pub ticket: String,
}

/// Body of [`ForgetPlayers`]: `server_id` was deleted or stopped following an instance. With
/// `share_id`, only while it still follows that share, so a late call never drops the players of
/// a share it follows since.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ForgottenServer {
    pub server_id: String,
    pub share_id: Option<String>,
}

/// A sign-in code as the user typed it. The dash and case do not matter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct EnteredCode {
    pub code: String,
}

/// A Core waiting for the account to approve its code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PendingCore {
    /// The machine's name, shown so the user can tell it is their Core.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ApprovedCore {
    pub id: String,
    pub name: String,
}

/// A Core signed in to the account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ListedCore {
    pub id: String,
    pub name: String,
    /// The Core's own version. Null until it has started once since it learned to report it.
    pub version: Option<String>,
    /// Where this Core is on its own network, reported by the Core itself.
    pub address: String,
    /// Where this Core is on the internet, once it has proven it can be reached there.
    pub public_address: Option<String>,
    /// SHA-256 of the certificate this Core serves HTTPS with. The App trusts only this
    /// certificate for this Core. Null for a Core that does not serve HTTPS yet.
    pub cert_fingerprint: Option<String>,
    pub created: String,
    pub last_seen: String,
}

/// A signed session ticket.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreTicket {
    pub ticket: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreId {
    pub id: String,
}

/// Body of [`CreateServerShare`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewServerShare {
    pub name: String,
    /// The server whose share this is.
    pub server_id: String,
}

crate::endpoints! {
    /// The backend's Core routes.
    group cores;

    /// Core, signed out: a code to show its owner and the key it will use once they approve. Asking
    /// again replaces the code.
    RequestSignInCode: Post "/core/auth/code", None, (), (), Json<NewSignInCode> => Json<SignInCode>;
    /// Core, signed out, with the key from [`RequestSignInCode`]: has the owner approved yet? 410
    /// `code_expired` once the code is gone.
    PollSignIn: Post "/core/auth/poll", CoreKey, (), (), Empty => Json<SignInStatus>;
    /// Core, signed out: a WebSocket the backend sends `approved` on and closes once the code is
    /// approved. Then the Core asks [`PollSignIn`].
    WaitForSignIn: Get "/core/auth/wait", CoreKey, (), (), Empty => Upgrade<String, ()>;
    /// Core: its whole state, on start and whenever it changes. A report that changes nothing writes
    /// nothing.
    ReportState: Post "/core/state", CoreKey, (), (), Json<StateReport> => Empty;
    /// Core: what its store holds, answered with what to drop and what to pull.
    ReportStorage: Post "/core/storage", CoreKey, (), (), Json<StorageReport> => Json<StorageAnswer>;
    /// Core: a server was deleted or stopped following an instance, so its players and links go.
    ForgetPlayers: Delete "/core/players", CoreKey, (), (), Json<ForgottenServer> => Empty;
    /// Core: the share of its server `server_id`, owned by its owner. The server's existing share is
    /// answered when it has one.
    CreateServerShare: Post "/core/shares", CoreKey, (), (), Json<NewServerShare> => Json<CreatedId>;
    /// Core: rename one of its servers' shares.
    RenameServerShare: Patch "/core/shares/{id}", CoreKey, SharedInstanceId, (), Json<SharedInstanceUpdate> => Empty;
    /// Core: delete one of its servers' shares, even while its owner may not share.
    DeleteServerShare: Delete "/core/shares/{id}", CoreKey, SharedInstanceId, (), Empty => Empty;
    /// Core: push a version of one of its servers' shares.
    CreateServerShareVersion: Post "/core/shares/{id}/versions", CoreKey, SharedInstanceId, (), Json<NewInstanceVersion> => Json<InstanceVersion>;
    /// Core: the latest version of a share its owner reads, without the owner's extras.
    GetLatestShareVersion: Get "/core/shares/{id}/versions", CoreKey, SharedInstanceId, (), Empty => Json<InstanceVersion>;
    /// Core: one version of a share its owner reads, without the owner's extras.
    GetShareVersion: Get "/core/shares/{id}/versions/{version}", CoreKey, InstanceVersionId, (), Empty => Json<InstanceVersion>;
    /// App: which machine shows this code, so the user can check before approving.
    LookUpSignInCode: Post "/cores/approve/lookup", Account, (), (), Json<EnteredCode> => Json<PendingCore>;
    /// App: approve the Core showing this code. It joins the account's Cores and its key starts
    /// working.
    ApproveSignInCode: Post "/cores/approve", Account, (), (), Json<EnteredCode> => Json<ApprovedCore>;
    /// App: the account's Cores.
    ListCores: Get "/cores", Account, (), (), Empty => Json<Vec<ListedCore>>;
    /// App: a session ticket for one of the account's Cores.
    IssueTicket: Post "/cores/{id}/ticket", Account, CoreId, (), Empty => Json<CoreTicket>;
    /// App: the owner takes a Core off the account. Its key stops working at once.
    RemoveCore: Delete "/cores/{id}", Account, CoreId, (), Empty => Empty;
    /// The backend's ticket keys. A static file, refetched daily and when a ticket names an unknown
    /// key.
    Keys: Get "/core/keys.json", None, (), (), Empty => Json<PublicKeys>;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    let declarations = [
        NewSignInCode::decl(&cfg),
        SignInCode::decl(&cfg),
        SignInStatus::decl(&cfg),
        CoreOwner::decl(&cfg),
        StateReport::decl(&cfg),
        StorageReport::decl(&cfg),
        HeldVersion::decl(&cfg),
        StorageAnswer::decl(&cfg),
        PullOrder::decl(&cfg),
        ForgottenServer::decl(&cfg),
        EnteredCode::decl(&cfg),
        PendingCore::decl(&cfg),
        ApprovedCore::decl(&cfg),
        ListedCore::decl(&cfg),
        CoreTicket::decl(&cfg),
        CoreId::decl(&cfg),
        NewServerShare::decl(&cfg),
    ]
    .map(|declaration| format!("\nexport {declaration}\n"))
    .concat();
    declarations + &ts_module().ts(false)
}
