//! The backend's Core routes. A Core signs in with a code its owner approves in the App, then
//! calls with its own key (`Bearer amc_...`): it reports its state on start and on change, and its
//! storage after uploads, pulls and hourly. A key the backend no longer knows gets 410
//! `core_removed`, and the Core un-pairs. The App lists the account's Cores, approves codes and
//! gets the tickets it signs in to a Core with.

use serde::{Deserialize, Serialize};

use crate::{
    Auth, Empty, Endpoint, Json, Method, Upgrade, core::CoreRole,
    tickets::PublicKeys,
};

/// Body of [`RequestSignInCode`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SignInCode {
    pub code: String,
    pub key: String,
    /// Seconds.
    pub expires_in: i64,
    /// Seconds.
    pub poll_interval: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SignInStatus {
    Pending,
    Approved { account: CoreOwner },
}

/// The account that approved a Core's code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CoreOwner {
    pub id: String,
    pub username: String,
}

/// Body of [`ReportState`]: everything the backend keeps about a Core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StateReport {
    pub version: String,
    /// Where the App reaches this Core on its own network, `https://ip:port`.
    pub address: String,
    /// Where the App reaches it from outside, once known.
    pub public_address: Option<String>,
    /// SHA-256 of the TLS certificate, lowercase hex. The App pins it.
    pub cert_fingerprint: String,
    /// The servers shown to the people their instance is shared with.
    pub servers: Vec<VisibleServer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VisibleServer {
    pub id: String,
    pub name: String,
    /// The Minecraft address, `host:port`.
    pub address: String,
    pub shared_instance_id: String,
}

/// Body of [`ReportStorage`]. Bytes throughout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HeldVersion {
    /// The version's hash.
    pub root: String,
    /// The bytes of its objects.
    pub bytes: u64,
}

/// What the backend answers a [`StorageReport`] with.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StorageAnswer {
    /// Versions to stop holding.
    #[serde(default)]
    pub drop: Vec<String>,
    /// Versions to copy from another Core.
    #[serde(default)]
    pub pull: Vec<PullOrder>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PullOrder {
    pub root: String,
    /// A pull ticket naming the source Core.
    pub ticket: String,
}

/// Body of [`ReplaceAccess`]: everyone with a role on any server of the Core, so who may be told
/// its address. Nothing about roles; the owner is never listed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AccessDirectory {
    pub user_ids: Vec<String>,
}

/// Body of [`GrantAccess`]: the owner invited `user` to a server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AccessGrant {
    /// A user id or username.
    pub user: String,
    pub server_id: String,
    pub server_name: String,
    /// Shown in the invite.
    pub role: CoreRole,
}

/// Who an invite went to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GrantedUser {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

/// Body of [`RevokeAccess`]: `user_id` lost `server_id`. `keep_core` when they still have another
/// server on the Core, so they stay in its directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AccessRevocation {
    pub user_id: String,
    pub server_id: String,
    #[serde(default)]
    pub keep_core: bool,
}

/// Body of [`ForgetPlayers`]: `server_id` was deleted or stopped following an instance. With
/// `share_id`, only while it still follows that share, so a late call never drops the players of
/// a share it follows since.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ForgottenServer {
    pub server_id: String,
    pub share_id: Option<String>,
}

/// A sign-in code as the user typed it. The dash and case do not matter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EnteredCode {
    pub code: String,
}

/// A Core waiting for the account to approve its code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PendingCore {
    /// The machine's name, shown so the user can tell it is their Core.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ApprovedCore {
    pub id: String,
    pub name: String,
}

/// A Core the signed-in account may reach: one signed in to the account, or one whose owner
/// invited it to manage a server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ListedCore {
    pub id: String,
    pub name: String,
    /// False for a Core the account was only invited to. Its owner manages it.
    pub owned: bool,
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CoreTicket {
    pub ticket: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CoreId {
    pub id: String,
}

/// Core, signed out: a code to show its owner and the key it will use once they approve. Asking
/// again replaces the code.
pub struct RequestSignInCode;

/// Core, signed out, with the key from [`RequestSignInCode`]: has the owner approved yet? 410
/// `code_expired` once the code is gone.
pub struct PollSignIn;

/// Core, signed out: a WebSocket the backend sends `approved` on and closes once the code is
/// approved. Then the Core asks [`PollSignIn`].
pub struct WaitForSignIn;

/// Core: its whole state, on start and whenever it changes. A report that changes nothing writes
/// nothing.
pub struct ReportState;

/// Core: what its store holds, answered with what to drop and what to pull.
pub struct ReportStorage;

/// Core: its whole access directory, on every start and reconnect.
pub struct ReplaceAccess;

/// Core: the owner invited someone to a server. They may reach the Core and get the invite as a
/// notification.
pub struct GrantAccess;

/// Core: someone lost a server. Their invite is withdrawn, and their way to the Core unless
/// `keep_core`.
pub struct RevokeAccess;

/// Core: a server was deleted or stopped following an instance, so its players and links go.
pub struct ForgetPlayers;

/// App: which machine shows this code, so the user can check before approving.
pub struct LookUpSignInCode;

/// App: approve the Core showing this code. It joins the account's Cores and its key starts
/// working.
pub struct ApproveSignInCode;

/// App: the account's own Cores, then the ones it was invited to.
pub struct ListCores;

/// App: a session ticket for one Core, for its owner or anyone in its access directory.
pub struct IssueTicket;

/// App: the owner takes a Core off the account. Its key stops working at once.
pub struct RemoveCore;

/// The backend's ticket keys. A static file, refetched daily and when a ticket names an unknown
/// key.
pub struct Keys;

impl Endpoint for RequestSignInCode {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/core/auth/code";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<NewSignInCode>;
    type Response = Json<SignInCode>;
}

impl Endpoint for PollSignIn {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/core/auth/poll";
    const AUTH: Auth = Auth::CoreKey;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<SignInStatus>;
}

impl Endpoint for WaitForSignIn {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/core/auth/wait";
    const AUTH: Auth = Auth::CoreKey;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Upgrade<String, ()>;
}

impl Endpoint for ReportState {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/core/state";
    const AUTH: Auth = Auth::CoreKey;
    type Params = ();
    type Query = ();
    type Body = Json<StateReport>;
    type Response = Empty;
}

impl Endpoint for ReportStorage {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/core/storage";
    const AUTH: Auth = Auth::CoreKey;
    type Params = ();
    type Query = ();
    type Body = Json<StorageReport>;
    type Response = Json<StorageAnswer>;
}

impl Endpoint for ReplaceAccess {
    const METHOD: Method = Method::Put;
    const PATH: &'static str = "/core/access";
    const AUTH: Auth = Auth::CoreKey;
    type Params = ();
    type Query = ();
    type Body = Json<AccessDirectory>;
    type Response = Empty;
}

impl Endpoint for GrantAccess {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/core/access";
    const AUTH: Auth = Auth::CoreKey;
    type Params = ();
    type Query = ();
    type Body = Json<AccessGrant>;
    type Response = Json<GrantedUser>;
}

impl Endpoint for RevokeAccess {
    const METHOD: Method = Method::Delete;
    const PATH: &'static str = "/core/access";
    const AUTH: Auth = Auth::CoreKey;
    type Params = ();
    type Query = ();
    type Body = Json<AccessRevocation>;
    type Response = Empty;
}

impl Endpoint for ForgetPlayers {
    const METHOD: Method = Method::Delete;
    const PATH: &'static str = "/core/players";
    const AUTH: Auth = Auth::CoreKey;
    type Params = ();
    type Query = ();
    type Body = Json<ForgottenServer>;
    type Response = Empty;
}

impl Endpoint for LookUpSignInCode {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/cores/approve/lookup";
    const AUTH: Auth = Auth::Account;
    type Params = ();
    type Query = ();
    type Body = Json<EnteredCode>;
    type Response = Json<PendingCore>;
}

impl Endpoint for ApproveSignInCode {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/cores/approve";
    const AUTH: Auth = Auth::Account;
    type Params = ();
    type Query = ();
    type Body = Json<EnteredCode>;
    type Response = Json<ApprovedCore>;
}

impl Endpoint for ListCores {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/cores";
    const AUTH: Auth = Auth::Account;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<Vec<ListedCore>>;
}

impl Endpoint for IssueTicket {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/cores/{id}/ticket";
    const AUTH: Auth = Auth::Account;
    type Params = CoreId;
    type Query = ();
    type Body = Empty;
    type Response = Json<CoreTicket>;
}

impl Endpoint for RemoveCore {
    const METHOD: Method = Method::Delete;
    const PATH: &'static str = "/cores/{id}";
    const AUTH: Auth = Auth::Account;
    type Params = CoreId;
    type Query = ();
    type Body = Empty;
    type Response = Empty;
}

impl Endpoint for Keys {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/core/keys.json";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<PublicKeys>;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    use crate::ts::endpoint;

    let cfg = crate::ts::config();
    let declarations = [
        NewSignInCode::decl(&cfg),
        SignInCode::decl(&cfg),
        SignInStatus::decl(&cfg),
        CoreOwner::decl(&cfg),
        StateReport::decl(&cfg),
        VisibleServer::decl(&cfg),
        StorageReport::decl(&cfg),
        HeldVersion::decl(&cfg),
        StorageAnswer::decl(&cfg),
        PullOrder::decl(&cfg),
        AccessDirectory::decl(&cfg),
        AccessGrant::decl(&cfg),
        GrantedUser::decl(&cfg),
        AccessRevocation::decl(&cfg),
        ForgottenServer::decl(&cfg),
        EnteredCode::decl(&cfg),
        PendingCore::decl(&cfg),
        ApprovedCore::decl(&cfg),
        ListedCore::decl(&cfg),
        CoreTicket::decl(&cfg),
        CoreId::decl(&cfg),
    ]
    .map(|declaration| format!("\nexport {declaration}\n"))
    .concat();
    let endpoints = [
        ("RequestSignInCode", endpoint::<RequestSignInCode>()),
        ("PollSignIn", endpoint::<PollSignIn>()),
        ("WaitForSignIn", endpoint::<WaitForSignIn>()),
        ("ReportState", endpoint::<ReportState>()),
        ("ReportStorage", endpoint::<ReportStorage>()),
        ("ReplaceAccess", endpoint::<ReplaceAccess>()),
        ("GrantAccess", endpoint::<GrantAccess>()),
        ("RevokeAccess", endpoint::<RevokeAccess>()),
        ("ForgetPlayers", endpoint::<ForgetPlayers>()),
        ("LookUpSignInCode", endpoint::<LookUpSignInCode>()),
        ("ApproveSignInCode", endpoint::<ApproveSignInCode>()),
        ("ListCores", endpoint::<ListCores>()),
        ("IssueTicket", endpoint::<IssueTicket>()),
        ("RemoveCore", endpoint::<RemoveCore>()),
        ("Keys", endpoint::<Keys>()),
    ]
    .map(|(name, endpoint)| format!("\t{name}: {endpoint},\n"))
    .concat();
    format!(
        "{declarations}\n/** The backend's Core routes. */\nexport const cores = {{\n{endpoints}}}\n"
    )
}
