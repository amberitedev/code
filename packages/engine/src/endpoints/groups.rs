use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use crate::instance::{InstanceGroup, InstanceGroupMembershipUpdate};

contracts::endpoints! {
    group instanceGroups;

    /// The Library's groups, in their order.
    ListInstanceGroups: Get "/instance-groups", Session, (), (), Empty => Json<Vec<InstanceGroup>>, client_only;
    CreateInstanceGroup: Post "/instance-groups", Session, (), (), Json<NewInstanceGroup> => Json<InstanceGroup>, client_only;
    RenameInstanceGroup: Patch "/instance-groups/{id}", Session, InstanceGroupRef, (), Json<InstanceGroupUpdate> => Json<InstanceGroup>, client_only;
    /// Its instances stay, outside it.
    DeleteInstanceGroup: Delete "/instance-groups/{id}", Session, InstanceGroupRef, (), Empty => Empty, client_only;
    /// Orders the groups as `ids` lists them.
    SetInstanceGroupOrder: Put "/instance-groups/order", Session, (), (), Json<Vec<String>> => Empty, client_only;
    /// Replaces the groups of each instance named.
    SetInstanceGroupMemberships: Put "/instance-groups/memberships", Session, (), (), Json<Vec<InstanceGroupMembershipUpdate>> => Empty, client_only;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceGroupRef {
    pub id: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewInstanceGroup {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceGroupUpdate {
    pub name: String,
}
