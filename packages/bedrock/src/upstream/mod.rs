//! Clones of upstream code too tangled with app-lib's `State` to borrow. Each is declared in
//! `ports.toml` with the upstream path and commit it came from; the App keeps the originals.

pub mod jar;

/// The content store clone's glue and public face. The clone itself is mounted at
/// `crate::state::content_store`, the path its files name it by.
#[path = "store/glue.rs"]
pub mod store;
