//! Free disk space, which the store checks before every upload and Copal before every backup.

use std::{io, path::Path};

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
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> =
        path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0u64;
    // SAFETY: a NUL-terminated path and a valid out pointer; the others may be null.
    let done = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &raw mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if done == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(available)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_free_space_of_a_real_disk_is_read() {
        let dir = tempfile::tempdir().unwrap();
        assert!(super::free(dir.path()).unwrap() > 0);
    }
}
