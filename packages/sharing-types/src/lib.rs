//! What the App and Core share about sharing (`docs/internals/pool.md`): the pool's encryption,
//! and the client for a Core's store routes that both use to reach a version's files.

pub mod crypto;
pub mod store;
