//! The backend's shared instance routes, on the paths and in the shapes of upstream's
//! `sharedinstances` v1 API wherever upstream calls them. A share's metadata lives here; its files
//! live encrypted on Cores' stores (`crate::store`), which a version names by its hash.
//!
//! An account calls with its session. A Core calls a few of them with its key, as the owner of its
//! own server's share: create (with `server_id`), rename, delete and its versions. It also reads
//! the versions of a share one of its servers follows, for the user whose read ticket it sends.
//! A share that is gone answers 404, one the caller can no longer read 401.

use modrinth_content_management::icons::IconConfig;
use serde::{Deserialize, Serialize};

use crate::{
    Auth, Empty, Endpoint, Hash, Json, Method, Raw, tickets::StoreHost,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedInstanceId {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InstanceInviteId {
    pub id: String,
    pub invite_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InstanceVersionId {
    pub id: String,
    pub version: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InviteId {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IconUpload {
    pub id: String,
    pub upload: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BlacklistedUser {
    pub user_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModeratedFile {
    pub id: String,
    pub version: i32,
    pub file: String,
}

/// A server of a Core, by both ids.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CoreServerId {
    pub core_id: String,
    pub server_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerInviteId {
    pub core_id: String,
    pub server_id: String,
    pub invite_id: String,
}

/// Query of [`ListSharedInstances`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedInstanceFilter {
    /// Only the caller's own id is allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub user: Option<String>,
    /// Also the shares the caller joined or plays through a server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub include_joined: Option<bool>,
}

/// Body of [`CreateSharedInstance`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewSharedInstance {
    pub name: String,
    /// A Core key only: the server whose share this is. Its existing share is answered when it
    /// has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub server_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreatedId {
    pub id: String,
}

/// Body of [`UpdateSharedInstance`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedInstanceUpdate {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedInstance {
    pub name: String,
    /// The URL of an uploaded image. Null for an icon editor icon, which `icon_config` holds.
    pub icon: Option<String>,
    pub icon_config: Option<IconConfig>,
    pub quarantine: bool,
    /// Someone other than the owner joined it once.
    pub installed_once: bool,
    /// For a player, the server they came through, which the App adds to the instance's list.
    pub linked_server: Option<LinkedServer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LinkedServer {
    /// The Minecraft address, `host:port`.
    pub domain: String,
    pub region: String,
    pub online_status: Option<OnlineStatus>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum OnlineStatus {
    Unknown,
    Running,
    Stopped,
}

/// The people a share is shared with, or the players of a server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InstanceUsers {
    pub users: Vec<InstanceUser>,
    /// Invite links that still work.
    pub tokens: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InstanceUser {
    pub id: String,
    /// RFC 3339. Null while the invite is pending.
    pub joined_at: Option<String>,
    pub join_type: JoinType,
    /// RFC 3339.
    pub last_played: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum JoinType {
    Owner,
    Invite,
    Link,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UserIds {
    pub user_ids: Vec<String>,
}

/// The invited accounts whose privacy settings refuse sharing invitations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FailedInvites {
    pub failed: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InviteLink {
    pub id: String,
    /// RFC 3339.
    pub expiration: String,
    /// 0 for no limit.
    pub max_uses: u32,
    pub uses: u32,
}

/// Body of [`CreateInvite`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewInviteLink {
    /// Seconds, a week when absent. 0 is a year.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_age: Option<u32>,
    /// 0, the default, for no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_uses: Option<u32>,
}

/// What an invite link shares, shown to anyone before they accept it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Invite {
    pub instance_id: String,
    pub instance_name: String,
    pub instance_icon: Option<String>,
    pub instance_icon_config: Option<IconConfig>,
    /// Empty while no version is ready.
    pub game_version: String,
    pub loader_version: String,
    pub inviter: Option<Inviter>,
    pub managers: Vec<InviteManager>,
    /// Empty for a server's link: players never see the share's members.
    pub instance_users: Vec<InviteUser>,
    /// With `?version=true`: the latest ready version, without file keys or download.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub version: Option<InstanceVersion>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Inviter {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InviteManager {
    User {
        id: String,
        name: String,
        avatar: Option<String>,
    },
    Server {
        name: String,
        icon: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InviteUser {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub joined_at: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InvitePreviewQuery {
    /// Adds the latest ready version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub version: Option<bool>,
}

/// A file of a modpack that members do not get: by its Modrinth version, or, for a file Modrinth
/// does not know, by its folder and file name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemovedFile {
    Version { version_id: String },
    Path { parent: String, filename: String },
}

/// A version's upstream fields, which a push sends beside the files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionContent {
    /// The Modrinth versions of its content.
    pub modrinth_ids: Vec<String>,
    /// The Modrinth version of the modpack it is based on.
    pub modpack_id: Option<String>,
    /// Files of the modpack that members do not get. Only with a modpack.
    pub removed_files: Vec<RemovedFile>,
    pub game_version: String,
    /// `vanilla`, `fabric`, `forge`, `quilt` or `neoforge`.
    pub loader: String,
    /// Empty for vanilla.
    pub loader_version: String,
}

/// A version as the backend stores it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionManifest {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub content: VersionContent,
    /// The hash of its sorted distinct object hashes, which names it on the stores.
    pub version_hash: Hash,
    pub external_files: Vec<ExternalFile>,
}

/// A file of a version that lives on the stores. Content files (mods and the like) are named by
/// their file name, every other file by its path in the instance or server folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ExternalFile {
    pub file_name: String,
    pub file_type: String,
    /// Always null: the file is fetched from the stores by `object`.
    #[serde(default)]
    pub url: Option<String>,
    pub file_size: u64,
    /// What a member's install compares to skip a file it already has.
    pub sha1: String,
    /// The SHA-256 of the plain bytes.
    pub sha256: Hash,
    /// The SHA-256 of the encrypted object, which names it on the stores.
    pub object: Hash,
    pub object_size: u64,
    /// The file's own key. Absent for content files, whose key derives from `sha256`, and in an
    /// invite's preview, which carries no keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub key: Option<Hash>,
}

/// One version of a share.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InstanceVersion {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub manifest: VersionManifest,
    pub version: i32,
    /// Its files are on the stores, so members can install it.
    pub ready: bool,
    /// For a member, once it is ready and has files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub download: Option<VersionDownload>,
    /// For its owner, while it is not ready.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub upload: Option<VersionUpload>,
    /// For its owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reports: Option<VersionReports>,
}

/// Where a ready version's files are, and the fetch ticket for them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionDownload {
    pub holders: Vec<StoreHost>,
    pub ticket: String,
}

/// The Core the owner uploads a version's files to, and the store ticket for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionUpload {
    pub core: StoreHost,
    pub ticket: String,
    /// RFC 3339.
    pub expires: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionReports {
    /// Members whose files decrypted to the wrong bytes.
    pub content: u32,
}

/// Body of [`CreateVersion`]. A push with the same files as a version still uploading resumes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewInstanceVersion {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub manifest: VersionManifest,
    /// The owner's Core that runs a server of this share, which then holds its files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub server_core: Option<String>,
}

/// Body of [`ReportVersion`]: a member got bad bytes of a version from `core_id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionFault {
    pub core_id: String,
    pub kind: FaultKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum FaultKind {
    /// The bytes did not hash to their object: the Core's fault.
    Object,
    /// They decrypted to another file: the manifest's, which only a new push fixes.
    Content,
}

/// A server someone else hosts for a shared instance and shows to its members.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HostedServer {
    pub core_id: String,
    pub server_id: String,
    pub name: String,
    /// The Minecraft address to ping and join. Not shown to the member.
    pub address: String,
    pub host: ServerHost,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerHost {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

/// Body of [`IssueReadTicket`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewReadTicket {
    pub core_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SignedReadTicket {
    pub ticket: String,
    /// RFC 3339.
    pub expires: String,
}

/// The account's shared storage: how much of its allowance on other people's Cores its shares
/// use, and the versions they keep. Bytes throughout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ShareStorage {
    pub used: u64,
    pub allowance: u64,
    /// Only the shares the account owns.
    pub shares: Vec<StoredShare>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoredShare {
    pub id: String,
    pub name: String,
    /// It is on someone else's Core, so it counts against the allowance.
    pub counted: bool,
    /// What it counts against the allowance, 0 when not counted.
    pub used: u64,
    /// What its kept versions take, counted or not.
    pub size: u64,
    /// The account's own Cores holding it.
    pub cores: Vec<String>,
    /// Newest first.
    pub versions: Vec<StoredVersion>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoredVersion {
    pub version: i32,
    /// RFC 3339.
    pub created: String,
    pub bytes: u64,
    pub ready: bool,
    /// The latest ready version, which members install. It cannot be deleted.
    pub current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BlacklistStatus {
    pub blacklisted: bool,
}

/// What a write to a server's players tells the backend about the server: the share it follows,
/// the name its players see, and where they join it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FollowedShare {
    pub instance_id: String,
    pub name: String,
    /// The server's game port on its Core.
    pub port: u16,
    /// Where the Core's router opened it on the internet. Null while it has not, which keeps the
    /// last one known.
    pub outside: Option<OutsideAddress>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OutsideAddress {
    pub ip: String,
    pub port: u16,
}

/// Body of [`InvitePlayers`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewPlayers {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub server: FollowedShare,
    pub user_ids: Vec<String>,
}

/// Body of [`CreatePlayerLink`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewPlayerLink {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub server: FollowedShare,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub link: NewInviteLink,
}

/// The caller's own shares, and with `include_joined` every share they can play too.
pub struct ListSharedInstances;
/// A new share owned by the caller, or by a Core key the share of its server `server_id`.
pub struct CreateSharedInstance;
/// Pending invitees may read it too, to preview the invite.
pub struct GetSharedInstance;
/// The owner, or the Core whose server's share it is.
pub struct UpdateSharedInstance;
/// The owner, or the Core whose server's share it is. Its files go once no version names them.
pub struct DeleteSharedInstance;
/// Joined members only; players get the content, not who it is shared with.
pub struct ListInstanceUsers;
/// The owner invites accounts.
pub struct InviteInstanceUsers;
/// The owner removes anyone; a member may remove only themselves, which also leaves the servers
/// they played it through. Their servers stop passing it on.
pub struct RemoveInstanceUsers;
/// The owner's live links.
pub struct ListInvites;
pub struct CreateInvite;
pub struct DeleteInvite;
/// Join through a link. Joining a share one already has is done.
pub struct AcceptInvite;
/// Accept every pending invite to the share, as a member and as a player. 404 when there is none.
pub struct AcceptPendingInvite;
pub struct DeclinePendingInvite;
/// Anyone, signed in or not.
pub struct GetInvite;
/// The newest ready version, or the one uploading while none is ready.
pub struct GetLatestVersion;
/// Members see a version once it is ready.
pub struct GetVersion;
/// The owner pushes: every file is already encrypted and named, and the answer names the Core to
/// upload them to. The version is ready once that Core confirms it holds them.
pub struct CreateVersion;
/// The owner deletes an older version. The current one and one still uploading stay.
pub struct DeleteVersion;
/// A member got bad bytes. The report is shown to the owner; nothing is marked broken.
pub struct ReportVersion;
/// The owner uploads an image.
pub struct SetIcon;
/// The owner sets an icon editor icon: only its settings, which clients draw themselves.
pub struct SetGeneratedIcon;
pub struct RemoveIcon;
/// An uploaded icon. Every upload gets a new URL, so it never changes.
pub struct GetIcon;
/// Members: the servers other people host for the share. The caller's own Cores are left out.
pub struct ListHostedServers;
/// Owner or member: a ticket for one Core to read the share for the caller, so a server they
/// manage can follow it.
pub struct IssueReadTicket;
/// The caller's shared storage.
pub struct GetShareStorage;
pub struct GetBlacklistStatus;
/// Moderators.
pub struct BlacklistUsers;
/// Moderators.
pub struct UnblacklistUsers;
/// Moderators. Files are opaque to the backend, so the share is quarantined instead.
pub struct DeleteModeratedFile;
/// The players of a Core's server: the people it is shared with, who read the share it follows
/// without being its members. For the Core's owner, its managers and the share's owner.
pub struct ListPlayers;
/// The Core's owner invites players, saying which share the server follows.
pub struct InvitePlayers;
/// A player may leave; the Core's owner and the share's owner remove anyone.
pub struct RemovePlayers;
/// The Core's owner: links that make whoever opens them a player.
pub struct ListPlayerLinks;
pub struct CreatePlayerLink;
pub struct DeletePlayerLink;

macro_rules! endpoints {
    ($($name:ident: $method:ident $path:literal, $auth:ident, $params:ty, $query:ty, $body:ty => $response:ty;)+) => {
        $(impl Endpoint for $name {
            const METHOD: Method = Method::$method;
            const PATH: &'static str = $path;
            const AUTH: Auth = Auth::$auth;
            type Params = $params;
            type Query = $query;
            type Body = $body;
            type Response = $response;
        })+

        #[cfg(test)]
        fn endpoint_entries() -> String {
            [$(format!("\t{}: {},\n", stringify!($name), crate::ts::endpoint::<$name>())),+].concat()
        }
    };
}

endpoints! {
    ListSharedInstances: Get "/v1/instances", Account, (), SharedInstanceFilter, Empty => Json<Vec<String>>;
    CreateSharedInstance: Post "/v1/instances", Account, (), (), Json<NewSharedInstance> => Json<CreatedId>;
    GetSharedInstance: Get "/v1/instances/{id}", Account, SharedInstanceId, (), Empty => Json<SharedInstance>;
    UpdateSharedInstance: Patch "/v1/instances/{id}", Account, SharedInstanceId, (), Json<SharedInstanceUpdate> => Empty;
    DeleteSharedInstance: Delete "/v1/instances/{id}", Account, SharedInstanceId, (), Empty => Empty;
    ListInstanceUsers: Get "/v1/instances/{id}/users", Account, SharedInstanceId, (), Empty => Json<InstanceUsers>;
    InviteInstanceUsers: Post "/v1/instances/{id}/users", Account, SharedInstanceId, (), Json<UserIds> => Json<FailedInvites>;
    RemoveInstanceUsers: Delete "/v1/instances/{id}/users", Account, SharedInstanceId, (), Json<UserIds> => Empty;
    ListInvites: Get "/v1/instances/{id}/invites", Account, SharedInstanceId, (), Empty => Json<Vec<InviteLink>>;
    CreateInvite: Post "/v1/instances/{id}/invites", Account, SharedInstanceId, (), Json<NewInviteLink> => Json<CreatedId>;
    AcceptPendingInvite: Post "/v1/instances/{id}/invites/pending", Account, SharedInstanceId, (), Empty => Empty;
    DeclinePendingInvite: Delete "/v1/instances/{id}/invites/pending", Account, SharedInstanceId, (), Empty => Empty;
    AcceptInvite: Post "/v1/instances/{id}/invites/{invite_id}", Account, InstanceInviteId, (), Empty => Empty;
    DeleteInvite: Delete "/v1/instances/{id}/invites/{invite_id}", Account, InstanceInviteId, (), Empty => Empty;
    GetInvite: Get "/v1/invites/{id}", None, InviteId, InvitePreviewQuery, Empty => Json<Invite>;
    GetLatestVersion: Get "/v1/instances/{id}/versions", Account, SharedInstanceId, (), Empty => Json<InstanceVersion>;
    CreateVersion: Post "/v1/instances/{id}/versions", Account, SharedInstanceId, (), Json<NewInstanceVersion> => Json<InstanceVersion>;
    GetVersion: Get "/v1/instances/{id}/versions/{version}", Account, InstanceVersionId, (), Empty => Json<InstanceVersion>;
    DeleteVersion: Delete "/v1/instances/{id}/versions/{version}", Account, InstanceVersionId, (), Empty => Empty;
    ReportVersion: Post "/v1/instances/{id}/versions/{version}/reports", Account, InstanceVersionId, (), Json<VersionFault> => Empty;
    SetIcon: Put "/v1/instances/{id}/icon", Account, SharedInstanceId, (), Raw => Empty;
    SetGeneratedIcon: Put "/v1/instances/{id}/icon/generated", Account, SharedInstanceId, (), Json<IconConfig> => Empty;
    RemoveIcon: Delete "/v1/instances/{id}/icon", Account, SharedInstanceId, (), Empty => Empty;
    GetIcon: Get "/v1/icons/{id}/{upload}", None, IconUpload, (), Empty => Raw;
    ListHostedServers: Get "/v1/instances/{id}/servers", Account, SharedInstanceId, (), Empty => Json<Vec<HostedServer>>;
    IssueReadTicket: Post "/v1/instances/{id}/server-ticket", Account, SharedInstanceId, (), Json<NewReadTicket> => Json<SignedReadTicket>;
    GetShareStorage: Get "/v1/storage", Account, (), (), Empty => Json<ShareStorage>;
    GetBlacklistStatus: Get "/v1/blacklist/{user_id}", Account, BlacklistedUser, (), Empty => Json<BlacklistStatus>;
    BlacklistUsers: Post "/v1/moderation/blacklist", Account, (), (), Json<UserIds> => Empty;
    UnblacklistUsers: Delete "/v1/moderation/blacklist", Account, (), (), Json<UserIds> => Empty;
    DeleteModeratedFile: Delete "/v1/moderation/instances/{id}/versions/{version}/files/{file}", Account, ModeratedFile, (), Empty => Empty;
    ListPlayers: Get "/v1/servers/{core_id}/{server_id}/players", Account, CoreServerId, (), Empty => Json<InstanceUsers>;
    InvitePlayers: Post "/v1/servers/{core_id}/{server_id}/players", Account, CoreServerId, (), Json<NewPlayers> => Json<FailedInvites>;
    RemovePlayers: Delete "/v1/servers/{core_id}/{server_id}/players", Account, CoreServerId, (), Json<UserIds> => Empty;
    ListPlayerLinks: Get "/v1/servers/{core_id}/{server_id}/invites", Account, CoreServerId, (), Empty => Json<Vec<InviteLink>>;
    CreatePlayerLink: Post "/v1/servers/{core_id}/{server_id}/invites", Account, CoreServerId, (), Json<NewPlayerLink> => Json<CreatedId>;
    DeletePlayerLink: Delete "/v1/servers/{core_id}/{server_id}/invites/{invite_id}", Account, ServerInviteId, (), Empty => Empty;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    let declarations = [
        IconConfig::decl(&cfg),
        <modrinth_content_management::icons::IconBackground as TS>::decl(&cfg),
        SharedInstanceId::decl(&cfg),
        InstanceInviteId::decl(&cfg),
        InstanceVersionId::decl(&cfg),
        InviteId::decl(&cfg),
        IconUpload::decl(&cfg),
        BlacklistedUser::decl(&cfg),
        ModeratedFile::decl(&cfg),
        CoreServerId::decl(&cfg),
        ServerInviteId::decl(&cfg),
        SharedInstanceFilter::decl(&cfg),
        NewSharedInstance::decl(&cfg),
        CreatedId::decl(&cfg),
        SharedInstanceUpdate::decl(&cfg),
        SharedInstance::decl(&cfg),
        LinkedServer::decl(&cfg),
        OnlineStatus::decl(&cfg),
        InstanceUsers::decl(&cfg),
        InstanceUser::decl(&cfg),
        JoinType::decl(&cfg),
        UserIds::decl(&cfg),
        FailedInvites::decl(&cfg),
        InviteLink::decl(&cfg),
        NewInviteLink::decl(&cfg),
        Invite::decl(&cfg),
        Inviter::decl(&cfg),
        InviteManager::decl(&cfg),
        InviteUser::decl(&cfg),
        InvitePreviewQuery::decl(&cfg),
        RemovedFile::decl(&cfg),
        VersionContent::decl(&cfg),
        VersionManifest::decl(&cfg),
        ExternalFile::decl(&cfg),
        InstanceVersion::decl(&cfg),
        VersionDownload::decl(&cfg),
        VersionUpload::decl(&cfg),
        VersionReports::decl(&cfg),
        NewInstanceVersion::decl(&cfg),
        VersionFault::decl(&cfg),
        FaultKind::decl(&cfg),
        HostedServer::decl(&cfg),
        ServerHost::decl(&cfg),
        NewReadTicket::decl(&cfg),
        SignedReadTicket::decl(&cfg),
        ShareStorage::decl(&cfg),
        StoredShare::decl(&cfg),
        StoredVersion::decl(&cfg),
        BlacklistStatus::decl(&cfg),
        FollowedShare::decl(&cfg),
        OutsideAddress::decl(&cfg),
        NewPlayers::decl(&cfg),
        NewPlayerLink::decl(&cfg),
    ]
    .map(|declaration| format!("\nexport {declaration}\n"))
    .concat();
    format!(
        "{declarations}\n/** The backend's shared instance routes, on upstream's `sharedinstances` v1 paths. */\nexport const sharing = {{\n{}}}\n",
        endpoint_entries()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_reads_as_the_backend_splices_it() {
        let hash = "ab".repeat(32);
        let body = serde_json::json!({
            "modrinth_ids": ["v1"],
            "modpack_id": null,
            "removed_files": [],
            "game_version": "1.21.1",
            "loader": "fabric",
            "loader_version": "0.16.0",
            "version_hash": hash,
            "external_files": [{
                "file_name": "config/a.toml",
                "file_type": "config",
                "url": null,
                "file_size": 1,
                "sha1": "s",
                "sha256": hash,
                "object": hash,
                "object_size": 17,
                "key": hash,
            }],
            "version": 3,
            "ready": true,
        });
        let version: InstanceVersion = serde_json::from_value(body).unwrap();
        assert_eq!(version.version, 3);
        assert_eq!(version.manifest.content.loader, "fabric");
        assert_eq!(
            version.manifest.external_files[0].key,
            Some(Hash([0xab; 32]))
        );
        assert!(version.download.is_none());
    }
}
