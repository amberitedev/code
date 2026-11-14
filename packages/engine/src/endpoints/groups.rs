use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::endpoints;
use crate::instance::{InstanceGroup, InstanceGroupMembershipUpdate};

endpoints! {
    /// The Library's groups, in their order.
    ListInstanceGroups: Get "/instance-groups", (), (), Empty => Json<Vec<InstanceGroup>>;
    CreateInstanceGroup: Post "/instance-groups", (), (), Json<NewInstanceGroup> => Json<InstanceGroup>;
    RenameInstanceGroup: Patch "/instance-groups/{id}", InstanceGroupRef, (), Json<InstanceGroupUpdate> => Json<InstanceGroup>;
    /// Its instances stay, outside it.
    DeleteInstanceGroup: Delete "/instance-groups/{id}", InstanceGroupRef, (), Empty => Empty;
    /// Orders the groups as `ids` lists them.
    SetInstanceGroupOrder: Put "/instance-groups/order", (), (), Json<Vec<String>> => Empty;
    /// Replaces the groups of each instance named.
    SetInstanceGroupMemberships: Put "/instance-groups/memberships", (), (), Json<Vec<InstanceGroupMembershipUpdate>> => Empty;
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
