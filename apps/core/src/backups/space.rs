//! Free disk space, checked before anything big is written (a backup, a restore, a pack, an
//! unpacked archive), so a full disk refuses it up front instead of failing halfway.

pub use sharing_storage::space::free;

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;

/// Not enough room on the Core's disk for what was asked.
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

/// Refuse writing `needed` bytes unless the disk keeps some room after it: a twentieth of what
/// is written, and at least 256 MB, so it never fills the disk a server writes to.
pub fn enough_space(needed: u64, free: u64) -> Result<(), NoSpace> {
    let wanted = needed.saturating_add((needed / 20).max(256 * MB));
    if wanted > free {
        return Err(NoSpace {
            needed: wanted,
            free,
        });
    }
    Ok(())
}

/// A size as people read it: gigabytes with one decimal, or whole megabytes below one.
pub fn label(bytes: u64) -> String {
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else {
        format!("{} MB", bytes.div_ceil(MB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_that_would_fill_the_disk_is_refused() {
        assert!(enough_space(100 * MB, 1024 * MB).is_ok());
        // It fits, but leaves less than 256 MB.
        let error = enough_space(900 * MB, 1024 * MB).unwrap_err();
        assert_eq!(
            error.to_string(),
            "There is not enough disk space on the Core: this needs about 1.1 GB and 1.0 GB is free."
        );
        // Big writes keep a twentieth free.
        assert!(enough_space(100_000 * MB, 104_000 * MB).is_err());
        assert!(enough_space(100_000 * MB, 106_000 * MB).is_ok());
    }
}
