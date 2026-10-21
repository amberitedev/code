//! Free disk space, checked before every backup so a full disk refuses it instead of failing
//! halfway.

use std::{io, path::Path};

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

/// The bytes this account may still write on the disk that holds `path`.
#[cfg(unix)]
pub fn free(path: &Path) -> io::Result<u64> {
    let stat = nix::sys::statvfs::statvfs(path)?;
    // The field types differ between platforms.
    #[allow(clippy::useless_conversion)]
    Ok(u64::from(stat.blocks_available()) * u64::from(stat.fragment_size()))
}

/// The bytes this account may still write on the disk that holds `path`.
#[cfg(windows)]
pub fn free(path: &Path) -> io::Result<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    use crate::daemon::win::{check, wide};
    let mut available = 0u64;
    // SAFETY: a NUL-terminated path and a valid out pointer; the others may be null.
    check(unsafe {
        GetDiskFreeSpaceExW(
            wide(path).as_ptr(),
            &raw mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    })?;
    Ok(available)
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

    #[test]
    fn the_free_space_of_a_real_disk_is_read() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free(dir.path()).unwrap() > 0);
    }
}
