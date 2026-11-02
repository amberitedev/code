//! Where a server's state lives outside Core's types: SQLite rows, the server folder on disk, and
//! the network services content comes from.

pub mod filesystem;
pub mod network;
pub mod sqlite;
