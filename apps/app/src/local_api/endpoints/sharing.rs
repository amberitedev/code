use contracts::{Empty, Json};
use engine::data::CachedSharedInstanceSummary;
use serde::{Deserialize, Serialize};

endpoints! {
    /// Whether the signed-in account can share instances.
    CanUseSharedInstances: Get "/app/sharing/available", (), (), Empty => Json<bool>;
    /// The signed-in account's shares as last seen, empty when signed out.
    GetCachedSharedLibrary: Get "/app/sharing/library", (), (), Empty => Json<Vec<CachedSharedInstanceSummary>>;
    SetCachedSharedLibrary: Put "/app/sharing/library", (), (), Json<Vec<CachedSharedInstanceSummary>> => Empty;
    /// Publishes the instance's current content as its share's next version. Progress is bytes
    /// uploaded.
    PublishSharedInstance: Post "/app/ops/shared-instance-publish", (), (), Json<SharePublish> => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SharePublish {
    pub op: String,
    pub instance_id: String,
    /// The instance's files this version sends, by their path in the instance.
    pub file_paths: Vec<String>,
    /// Files the latest version shares that this one drops.
    pub removed_paths: Vec<String>,
    /// A Core the account owns that runs a server of this instance, which keeps the share's
    /// home copy.
    pub server_core_id: Option<String>,
}
