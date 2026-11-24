use contracts::{Empty, Json};

use crate::state::content_store::StoreUsage;

contracts::endpoints! {
    group contentStore;

    /// What the content store holds, shares and could free.
    GetStoreUsage: Get "/content-store/usage", Session, (), (), Empty => Json<StoreUsage>;
    /// Deletes cached files no instance uses, and answers the bytes freed.
    CleanStore: Post "/content-store/cleanup", Session, (), (), Empty => Json<u64>;
    /// The bytes unused cached files may take before cleanup deletes them.
    SetStoreCacheLimit: Put "/content-store/cache-limit", Session, (), (), Json<u64> => Empty;
}
