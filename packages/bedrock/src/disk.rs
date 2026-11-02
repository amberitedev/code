//! Free disk space, which the store checks before every upload and Copal before every backup,
//! download and unpack.

use std::{io, path::Path};

use crate::limits::DISK_MARGIN_BYTES;

/// The bytes a Core may still write on the disk that holds `path`: its free space less
/// [`DISK_MARGIN_BYTES`], which it always leaves free.
pub fn room(path: &Path) -> io::Result<u64> {
    Ok(free(path)?.saturating_sub(DISK_MARGIN_BYTES))
}

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;

/// A size as people read it: gigabytes with one decimal, or whole megabytes below one.
pub fn label(bytes: u64) -> String {
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else {
        format!("{} MB", bytes.div_ceil(MB))
    }
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
    fn sizes_read_as_megabytes_or_gigabytes() {
        assert_eq!(super::label(1), "1 MB");
        assert_eq!(super::label(900 * super::MB), "900 MB");
        assert_eq!(super::label(1156 * super::MB), "1.1 GB");
    }

    #[test]
    fn the_free_space_of_a_real_disk_is_read() {
        let dir = tempfile::tempdir().unwrap();
        assert!(super::free(dir.path()).unwrap() > 0);
    }
}
