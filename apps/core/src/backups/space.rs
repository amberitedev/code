//! Free disk space, checked before every backup so a full disk refuses it instead of failing
//! halfway.

pub use sharing_storage::space::free;

use super::Error;

const MB: u64 = 1024 * 1024;

/// Refuse a copy of `needed` bytes unless the disk keeps some room after it: a twentieth of the
/// copy, and at least 256 MB, so a backup never fills the disk a server writes to.
pub fn enough_space(needed: u64, free: u64) -> Result<(), Error> {
    let wanted = needed + (needed / 20).max(256 * MB);
    if wanted > free {
        return Err(Error::Space {
            needed_mb: wanted.div_ceil(MB),
            free_mb: free / MB,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backup_that_would_fill_the_disk_is_refused() {
        assert!(enough_space(100 * MB, 1024 * MB).is_ok());
        // The copy fits, but leaves less than 256 MB.
        assert!(matches!(
            enough_space(900 * MB, 1024 * MB),
            Err(Error::Space {
                needed_mb: 1156,
                free_mb: 1024
            })
        ));
        // Big worlds keep a twentieth free.
        assert!(enough_space(100_000 * MB, 104_000 * MB).is_err());
        assert!(enough_space(100_000 * MB, 106_000 * MB).is_ok());
    }
}
