//! Small helpers every part of Core uses.

use std::io;

/// A remove whose target was already gone counts as done: wrap `std::fs::remove_file`,
/// `remove_dir_all` and the like in it.
pub fn if_present(removed: io::Result<()>) -> io::Result<()> {
    match removed {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        removed => removed,
    }
}
