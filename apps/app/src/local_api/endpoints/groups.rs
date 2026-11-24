use std::collections::HashMap;

use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use crate::library::groups::{InstanceGroup, InstanceGroupMembershipUpdate};

contracts::endpoints! {
    group instanceGroups;

    /// The Library's groups, in their order.
    ListInstanceGroups: Get "/app/instance-groups", None, (), (), Empty => Json<Vec<InstanceGroup>>;
    /// Each instance's group ids, by instance id. An instance in no group is absent.
    ListInstanceGroupMemberships: Get "/app/instance-groups/memberships", None, (), (), Empty => Json<HashMap<String, Vec<String>>>;
    CreateInstanceGroup: Post "/app/instance-groups", None, (), (), Json<NewInstanceGroup> => Json<InstanceGroup>;
    RenameInstanceGroup: Patch "/app/instance-groups/{id}", None, InstanceGroupRef, (), Json<InstanceGroupUpdate> => Json<InstanceGroup>;
    /// Its instances stay, outside it.
    DeleteInstanceGroup: Delete "/app/instance-groups/{id}", None, InstanceGroupRef, (), Empty => Empty;
    /// Orders the groups as `ids` lists them.
    SetInstanceGroupOrder: Put "/app/instance-groups/order", None, (), (), Json<Vec<String>> => Empty;
    /// Replaces the groups of each instance named.
    SetInstanceGroupMemberships: Put "/app/instance-groups/memberships", None, (), (), Json<Vec<InstanceGroupMembershipUpdate>> => Empty;
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
