//! What the App and Core share about sharing (`docs/internals/pool.md`): the pool's encryption,
//! the client for a Core's store routes that both use to reach a version's files, the writer
//! that pushes a share's versions, and the reading side that takes them.

pub mod consume;
pub mod crypto;
pub mod push;
pub mod store;
