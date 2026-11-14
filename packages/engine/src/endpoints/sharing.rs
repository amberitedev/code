use contracts::{Empty, Json, backend::sharing::UserIds};
use serde::{Deserialize, Serialize};

use super::{
    endpoints,
    install::{SharedInstanceInviteRef, SharedInstanceRef},
    instances::{InstanceRef, SharedInstanceAttachment},
};
use crate::instance::{
    SharedInstanceInvite, SharedInstanceInviteLink,
    SharedInstancePublishPreview, SharedInstanceUsers,
};

endpoints! {
    /// Empty when the instance has no share.
    GetSharedInstanceUsers: Get "/instances/{id}/shared/users", InstanceRef, (), Empty => Json<SharedInstanceUsers>;
    InviteSharedInstanceUsers: Post "/instances/{id}/shared/users", InstanceRef, (), Json<UserIds> => Json<SharedInstanceUsers>;
    RemoveSharedInstanceUsers: Post "/instances/{id}/shared/users/remove", InstanceRef, (), Json<SharedUsersRemoval> => Json<SharedInstanceUsers>;
    ListSharedInstanceInvites: Get "/instances/{id}/shared/invites", InstanceRef, (), Empty => Json<Vec<SharedInstanceInvite>>;
    CreateSharedInstanceInviteLink: Post "/instances/{id}/shared/invites", InstanceRef, (), Json<NewSharedInviteLink> => Json<SharedInstanceInviteLink>;
    RevokeSharedInstanceInvite: Delete "/instances/{id}/shared/invites/{invite_id}", SharedInstanceInviteRef, (), Empty => Empty;
    /// `null` when the instance has no share.
    GetSharedInstancePublishPreview: Get "/instances/{id}/shared/publish-preview", InstanceRef, (), Empty => Json<Option<SharedInstancePublishPreview>>;
    /// A plain-text report of the instance's last publish, for a support request.
    GetSharedInstancePublishSupportDetails: Get "/instances/{id}/shared/publish-support-details", InstanceRef, (), Empty => Json<Option<String>>;
    /// The instance's share, created with its first version when it has none.
    EnsureSharedInstance: Put "/instances/{id}/shared", InstanceRef, (), Empty => Json<SharedInstanceAttachment>;
    /// Deletes the share for everyone.
    UnpublishSharedInstance: Delete "/instances/{id}/shared", InstanceRef, (), Empty => Empty;
    /// Detaches the instance from its share and keeps its files.
    UnlinkSharedInstance: Post "/instances/{id}/shared/unlink", InstanceRef, (), Empty => Empty;
    DismissSharedInstanceRestoredNotice: Delete "/instances/{id}/shared/restored-notice", InstanceRef, (), Empty => Empty;
    /// Deletes a share the caller owns but has no instance of.
    UnpublishRemoteSharedInstance: Delete "/shared-instances/{id}", SharedInstanceRef, (), Empty => Empty;
    LeaveRemoteSharedInstance: Post "/shared-instances/{id}/leave", SharedInstanceRef, (), Empty => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharedUsersRemoval {
    pub user_ids: Vec<String>,
    pub has_pending_recipients: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewSharedInviteLink {
    pub max_age_seconds: Option<i32>,
    pub max_uses: Option<i32>,
    /// The link this one replaces, deleted once it exists.
    pub replace_invite_id: Option<String>,
}
