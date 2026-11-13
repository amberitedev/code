use contracts::{Empty, Json};

use super::endpoints;
use crate::state::content_store::StoreUsage;

endpoints! {
    /// What the content store holds, shares and could free.
    GetStoreUsage: Get "/content-store/usage", (), (), Empty => Json<StoreUsage>;
    /// Deletes cached files no instance uses, and answers the bytes freed.
    CleanStore: Post "/content-store/cleanup", (), (), Empty => Json<u64>;
    /// The bytes unused cached files may take before cleanup deletes them.
    SetStoreCacheLimit: Put "/content-store/cache-limit", (), (), Json<u64> => Empty;
}
