//! Free disk space, checked before every backup and restore so a full disk refuses it up front
//! instead of failing halfway.

use contracts::limits::DISK_MARGIN_BYTES;
pub use sharing_client::disk::free;
use sharing_client::disk::label;

/// Not enough room on the Core's disk for a backup or restore.
#[derive(Debug, thiserror::Error)]
#[error(
    "There is not enough disk space on the Core: this needs about {} and {} is free.",
    label(*.needed),
    label(*.free)
)]
pub struct NoSpace {
    pub needed: u64,
    pub free: u64,
}

/// Refuse a copy of `needed` bytes unless the disk keeps some room after it: a twentieth of the
/// copy, and at least [`DISK_MARGIN_BYTES`], so a backup never fills the disk a server writes to.
pub fn enough_space(needed: u64, free: u64) -> Result<(), NoSpace> {
    let wanted = needed.saturating_add((needed / 20).max(DISK_MARGIN_BYTES));
    if wanted > free {
        return Err(NoSpace {
            needed: wanted,
            free,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    #[test]
    fn a_backup_that_would_fill_the_disk_is_refused() {
        assert!(enough_space(100 * MB, 1024 * MB).is_ok());
        // The copy fits, but leaves less than 256 MB.
        let error = enough_space(900 * MB, 1024 * MB).unwrap_err();
        assert_eq!(
            error.to_string(),
            "There is not enough disk space on the Core: this needs about 1.1 GB and 1.0 GB is free."
        );
        // Big worlds keep a twentieth free.
        assert!(enough_space(100_000 * MB, 104_000 * MB).is_err());
        assert!(enough_space(100_000 * MB, 106_000 * MB).is_ok());
    }
}
