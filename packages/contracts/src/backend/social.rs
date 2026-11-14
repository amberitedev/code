//! The backend's social routes: profiles, friends, blocks, avatars, preferences, notifications and
//! the friends socket. They keep labrinth's paths and shapes, so upstream's pages call them as they
//! call Modrinth; upstream calls them under `/v2`, `/v3` and `/_internal`, which the backend strips.

use std::collections::HashMap;

use ariadne::{
    networking::message::{ClientToServerMessage, ServerToClientMessage},
    users::UserStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Auth, Empty, Endpoint, Json, Method, Raw, Upgrade};

/// Something this backend never has, such as an account's projects: an empty list, or null.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Never {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Developer,
    Moderator,
    Admin,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UserCampaigns {
    pub pride_26: Option<Never>,
}

/// Shown to admins and moderators only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UserLock {
    pub locked_by: String,
    pub reason: String,
    pub created: String,
}

/// An account. The private fields are null unless it is the caller's own or the caller is an
/// admin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct User {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    pub raw_avatar_url: Option<String>,
    pub bio: Option<String>,
    pub created: String,
    pub role: Role,
    pub badges: u64,
    pub campaigns: UserCampaigns,
    pub auth_providers: Option<Vec<Never>>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub has_password: Option<bool>,
    pub has_totp: Option<bool>,
    pub payout_data: Option<Never>,
    pub stripe_customer_id: Option<Never>,
    pub github_id: Option<Never>,
    pub allow_friend_requests: Option<bool>,
    pub eligibility_verified_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<UserLock>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SearchUser {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

/// A friendship or a pending request. As in labrinth, `id` is the recipient and `friend_id` the
/// sender.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UserFriend {
    pub id: String,
    pub friend_id: String,
    pub accepted: bool,
    pub created: String,
}

/// Body of [`EditUser`]. A null `bio` clears it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UserUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub bio: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub allow_friend_requests: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AllProjects {
    pub projects: Vec<Never>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, never>"))]
    pub organizations: HashMap<String, Never>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
    Oled,
    Retro,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LayoutOption {
    Grid,
    Rows,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FriendPrivacy {
    None,
    Mutual,
    Everyone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum InvitePrivacy {
    None,
    Friends,
    Everyone,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AppearancePreferences {
    pub auto: bool,
    pub theme: Theme,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BehaviorPreferences {
    pub minimize_app: bool,
    pub refocus_on_game_close: bool,
    pub hide_right_sidebar: bool,
    pub show_jump_in: bool,
    pub compact_instance_cards: bool,
    pub show_play_time: bool,
    pub hide_nametag: bool,
    pub show_all_screenshots: bool,
    pub show_files_tab_in_instances: bool,
    pub show_worlds_tab_in_instances: bool,
    pub show_screenshots_tab_in_instances: bool,
    pub show_skin_selector_in_sidebar: bool,
    /// 0 to 255.
    pub quick_instance_count: u8,
    pub warn_on_unknown_modpacks: bool,
    pub skip_non_essential_warnings: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LocalizationPreferences {
    /// Letters, digits and dashes, at most 35.
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LayoutPreferences {
    pub mods: LayoutOption,
    pub plugins: LayoutOption,
    pub datapacks: LayoutOption,
    pub shaders: LayoutOption,
    pub resourcepacks: LayoutOption,
    pub modpacks: LayoutOption,
    pub servers: LayoutOption,
    pub users: LayoutOption,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SidebarPreferences {
    pub right_aligned_search: bool,
    pub left_aligned_content: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SocialPreferences {
    pub friend_privacy: FriendPrivacy,
    pub shared_instances_privacy: InvitePrivacy,
    pub hosting_access_privacy: InvitePrivacy,
}

/// An account's settings, kept with the account so they follow it to every installation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UserPreferences {
    pub appearance: AppearancePreferences,
    pub behavior: BehaviorPreferences,
    pub localization: LocalizationPreferences,
    pub layouts: LayoutPreferences,
    pub sidebars: SidebarPreferences,
    pub social: SocialPreferences,
}

/// Body of [`EditPreferences`]: the settings to change, by section. The rest keep their value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(
        type = "{ [S in keyof UserPreferences]?: Partial<UserPreferences[S]> }"
    )
)]
#[serde(transparent)]
pub struct PreferencesUpdate(
    pub HashMap<String, serde_json::Map<String, Value>>,
);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NotificationAction {
    pub name: String,
    /// The method and the route to call.
    pub action_route: (String, String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Notification {
    pub id: String,
    pub user_id: String,
    pub read: bool,
    pub created: String,
    #[cfg_attr(
        feature = "ts",
        ts(type = "{ type: string, [key: string]: unknown }")
    )]
    pub body: Value,
    pub name: String,
    pub text: String,
    pub link: String,
    pub actions: Vec<NotificationAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SharedInstanceList {
    SharedInstanceUsers,
    SharedInstanceServers,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedInstanceChangeBody {
    #[serde(rename = "type")]
    pub list: SharedInstanceList,
    pub shared_instance_id: String,
}

/// A shared instance's member list or the servers its members host for it changed, so an open
/// page fetches it again. Shaped like a notification so the App passes it on, but not stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedInstanceChange {
    pub body: SharedInstanceChangeBody,
}

/// What the backend sends on [`FriendsSocket`]: ariadne's messages, and the notifications and
/// changes the App tells apart by not reading them as ariadne's.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum FriendsSocketMessage {
    Friends(ServerToClientMessage),
    Notification(Notification),
    SharedInstanceChange(SharedInstanceChange),
}

/// A user id or username.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UserRef {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NotificationRef {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AvatarRevision {
    pub user_id: String,
    pub revision: String,
}

/// Ids as labrinth takes them: a JSON array of at most 100, in the query string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IdList {
    pub ids: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UserSearch {
    pub query: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IconFormat {
    /// `png`, `jpg`, `jpeg`, `gif` or `webp`.
    pub ext: String,
}

/// The account session, sent in the query since a browser socket cannot send headers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SocketCode {
    pub code: String,
}

/// Up to 20 accounts whose username starts with the query.
pub struct SearchUsers;

/// Accounts by id or username. Signed out works too.
pub struct GetUsers;

/// The account's friends and pending requests, both ways.
pub struct ListFriends;

/// The presence of each online friend.
pub struct ListFriendStatuses;

/// Sends a request, or accepts the one the other account sent.
pub struct AddFriend;

/// Ends a friendship or withdraws or declines a request.
pub struct RemoveFriend;

/// The ids the account blocked.
pub struct ListBlocks;

/// Blocks an account and ends any friendship with it.
pub struct BlockUser;

pub struct UnblockUser;

/// An avatar. Each upload gets a new revision, so it is cached for a day.
pub struct GetAvatar;

/// One account. Signed out works too.
pub struct GetUser;

pub struct ListUserProjects;

pub struct ListUserOrganizations;

pub struct ListUserCollections;

/// The owner only.
pub struct ListFollowedProjects;

/// The owner only.
pub struct ListAllProjects;

/// The owner only.
pub struct EditUser;

/// The owner deletes the account, its shared instances and its Cores.
pub struct DeleteUser;

pub struct GetPreferences;

/// Answers with every setting after the change.
pub struct EditPreferences;

/// A PNG, JPEG, GIF or WebP of at most 512 KiB.
pub struct ChangeAvatar;

pub struct RemoveAvatar;

/// The owner's notifications, newest first.
pub struct ListUserNotifications;

pub struct GetNotification;

pub struct ReadNotification;

pub struct DeleteNotification;

/// The caller's notifications among `ids`. Others' are left out.
pub struct GetNotifications;

pub struct ReadNotifications;

pub struct DeleteNotifications;

/// The App's live link: friend presence, friend requests, notifications and shared instance
/// changes. Upstream calls it `/_internal/launcher_socket`.
pub struct FriendsSocket;

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
    SearchUsers: Get "/users/search", None, (), UserSearch, Empty => Json<Vec<SearchUser>>;
    GetUsers: Get "/users", Account, (), IdList, Empty => Json<Vec<User>>;
    ListFriends: Get "/friends", Account, (), (), Empty => Json<Vec<UserFriend>>;
    ListFriendStatuses: Get "/friends/status", Account, (), (), Empty => Json<Vec<UserStatus>>;
    AddFriend: Post "/friend/{id}", Account, UserRef, (), Empty => Empty;
    RemoveFriend: Delete "/friend/{id}", Account, UserRef, (), Empty => Empty;
    ListBlocks: Get "/blocks", Account, (), (), Empty => Json<Vec<String>>;
    BlockUser: Post "/block/{id}", Account, UserRef, (), Empty => Empty;
    UnblockUser: Delete "/block/{id}", Account, UserRef, (), Empty => Empty;
    GetAvatar: Get "/avatars/{user_id}/{revision}", None, AvatarRevision, (), Empty => Raw;
    GetUser: Get "/user/{id}", Account, UserRef, (), Empty => Json<User>;
    ListUserProjects: Get "/user/{id}/projects", None, UserRef, (), Empty => Json<Vec<Never>>;
    ListUserOrganizations: Get "/user/{id}/organizations", None, UserRef, (), Empty => Json<Vec<Never>>;
    ListUserCollections: Get "/user/{id}/collections", None, UserRef, (), Empty => Json<Vec<Never>>;
    ListFollowedProjects: Get "/user/{id}/follows", Account, UserRef, (), Empty => Json<Vec<Never>>;
    ListAllProjects: Get "/user/{id}/all-projects", Account, UserRef, (), Empty => Json<AllProjects>;
    EditUser: Patch "/user/{id}", Account, UserRef, (), Json<UserUpdate> => Empty;
    DeleteUser: Delete "/user/{id}", Account, UserRef, (), Empty => Empty;
    GetPreferences: Get "/user/{id}/preferences", Account, UserRef, (), Empty => Json<UserPreferences>;
    EditPreferences: Patch "/user/{id}/preferences", Account, UserRef, (), Json<PreferencesUpdate> => Json<UserPreferences>;
    ChangeAvatar: Patch "/user/{id}/icon", Account, UserRef, IconFormat, Raw => Empty;
    RemoveAvatar: Delete "/user/{id}/icon", Account, UserRef, (), Empty => Empty;
    ListUserNotifications: Get "/user/{id}/notifications", Account, UserRef, (), Empty => Json<Vec<Notification>>;
    GetNotification: Get "/notification/{id}", Account, NotificationRef, (), Empty => Json<Notification>;
    ReadNotification: Patch "/notification/{id}", Account, NotificationRef, (), Empty => Empty;
    DeleteNotification: Delete "/notification/{id}", Account, NotificationRef, (), Empty => Empty;
    GetNotifications: Get "/notifications", Account, (), IdList, Empty => Json<Vec<Notification>>;
    ReadNotifications: Patch "/notifications", Account, (), IdList, Empty => Empty;
    DeleteNotifications: Delete "/notifications", Account, (), IdList, Empty => Empty;
    FriendsSocket: Get "/launcher_socket", Account, (), SocketCode, Empty => Upgrade<FriendsSocketMessage, ClientToServerMessage>;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    let declarations = [
        Never::decl(&cfg),
        Role::decl(&cfg),
        UserCampaigns::decl(&cfg),
        UserLock::decl(&cfg),
        User::decl(&cfg),
        SearchUser::decl(&cfg),
        UserFriend::decl(&cfg),
        UserUpdate::decl(&cfg),
        AllProjects::decl(&cfg),
        Theme::decl(&cfg),
        LayoutOption::decl(&cfg),
        FriendPrivacy::decl(&cfg),
        InvitePrivacy::decl(&cfg),
        AppearancePreferences::decl(&cfg),
        BehaviorPreferences::decl(&cfg),
        LocalizationPreferences::decl(&cfg),
        LayoutPreferences::decl(&cfg),
        SidebarPreferences::decl(&cfg),
        SocialPreferences::decl(&cfg),
        UserPreferences::decl(&cfg),
        PreferencesUpdate::decl(&cfg),
        NotificationAction::decl(&cfg),
        Notification::decl(&cfg),
        SharedInstanceList::decl(&cfg),
        SharedInstanceChangeBody::decl(&cfg),
        SharedInstanceChange::decl(&cfg),
        UserStatus::decl(&cfg),
        ServerToClientMessage::decl(&cfg),
        ClientToServerMessage::decl(&cfg),
        FriendsSocketMessage::decl(&cfg),
        UserRef::decl(&cfg),
        NotificationRef::decl(&cfg),
        AvatarRevision::decl(&cfg),
        IdList::decl(&cfg),
        UserSearch::decl(&cfg),
        IconFormat::decl(&cfg),
        SocketCode::decl(&cfg),
    ]
    .map(|declaration| format!("\nexport {declaration}\n"))
    .concat();
    format!(
        "{declarations}\n/** The backend's social routes, in labrinth's shapes. */\nexport const social = {{\n{}}}\n",
        endpoint_entries()
    )
}
