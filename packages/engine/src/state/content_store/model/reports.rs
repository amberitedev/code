use serde::Serialize;

#[derive(Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct StoreUsage {
    pub unique_bytes: u64,
    pub shared_bytes: u64,
    pub unused_cache_bytes: u64,
    pub estimated_saved_bytes: u64,
    pub private_copy_bytes: u64,
    pub object_count: usize,
    pub damaged_objects: usize,
    pub cache_limit_bytes: u64,
}
