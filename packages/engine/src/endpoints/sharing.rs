use contracts::{Empty, Json, backend::sharing::UserIds};
use serde::{Deserialize, Serialize};

use super::{
    install::{SharedInstanceInviteRef, SharedInstanceRef},
    instances::{InstanceRef, SharedInstanceAttachment},
};
use crate::instance::{
    SharedInstanceInvite, SharedInstanceInviteLink,
    SharedInstancePublishPreview, SharedInstanceUsers,
};

contracts::endpoints! {
    group instanceSharing;

    /// Empty when the instance has no share.
    GetSharedInstanceUsers: Get "/instances/{id}/shared/users", Session, InstanceRef, (), Empty => Json<SharedInstanceUsers>, client_only;
    InviteSharedInstanceUsers: Post "/instances/{id}/shared/users", Session, InstanceRef, (), Json<UserIds> => Json<SharedInstanceUsers>, client_only;
    RemoveSharedInstanceUsers: Post "/instances/{id}/shared/users/remove", Session, InstanceRef, (), Json<SharedUsersRemoval> => Json<SharedInstanceUsers>, client_only;
    ListSharedInstanceInvites: Get "/instances/{id}/shared/invites", Session, InstanceRef, (), Empty => Json<Vec<SharedInstanceInvite>>, client_only;
    CreateSharedInstanceInviteLink: Post "/instances/{id}/shared/invites", Session, InstanceRef, (), Json<NewSharedInviteLink> => Json<SharedInstanceInviteLink>, client_only;
    RevokeSharedInstanceInvite: Delete "/instances/{id}/shared/invites/{invite_id}", Session, SharedInstanceInviteRef, (), Empty => Empty, client_only;
    /// `null` when the instance has no share.
    GetSharedInstancePublishPreview: Get "/instances/{id}/shared/publish-preview", Session, InstanceRef, (), Empty => Json<Option<SharedInstancePublishPreview>>, shared;
    /// A plain-text report of the instance's last publish, for a support request.
    GetSharedInstancePublishSupportDetails: Get "/instances/{id}/shared/publish-support-details", Session, InstanceRef, (), Empty => Json<Option<String>>, client_only;
    /// The instance's share, created with its first version when it has none.
    EnsureSharedInstance: Put "/instances/{id}/shared", Session, InstanceRef, (), Empty => Json<SharedInstanceAttachment>, client_only;
    /// Deletes the share for everyone.
    UnpublishSharedInstance: Delete "/instances/{id}/shared", Session, InstanceRef, (), Empty => Empty, shared;
    /// Detaches the instance from its share and keeps its files.
    UnlinkSharedInstance: Post "/instances/{id}/shared/unlink", Session, InstanceRef, (), Empty => Empty, shared;
    DismissSharedInstanceRestoredNotice: Delete "/instances/{id}/shared/restored-notice", Session, InstanceRef, (), Empty => Empty, client_only;
    /// Deletes a share the caller owns but has no instance of.
    UnpublishRemoteSharedInstance: Delete "/shared-instances/{id}", Session, SharedInstanceRef, (), Empty => Empty, client_only;
    LeaveRemoteSharedInstance: Post "/shared-instances/{id}/leave", Session, SharedInstanceRef, (), Empty => Empty, client_only;
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
