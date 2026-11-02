//! The pool as readers and writers reach it: the client for Cores' store routes, and fetching a
//! version's files back from its holders.

mod fetch;
mod model;
mod store;

pub use fetch::fetch;
pub use model::{InvalidPoolEntry, PoolAccess, PoolFile};
pub use store::{
    Fault, FaultKind, FetchError, Have, ListedObject, PoolCore, PoolObject,
    StoreClient, StoreError, UploadOutcome,
};
