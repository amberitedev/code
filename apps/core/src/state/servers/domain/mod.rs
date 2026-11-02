//! Rules about a server's content that touch no state: how a pack is planned over the files on
//! disk, which sides an addon runs on, which files a server's share carries, and what restoring a
//! version of that share changes.

pub mod pack;
pub mod restore;
pub mod server_share;
pub mod sides;
