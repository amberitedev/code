//! Who a process is. A pid can be reused, so the shim records its pid together with its start
//! time and the boot it started in, and Core trusts a pid only when all three still match.

use std::io;

/// This boot of the machine. It changes on every restart.
pub fn boot_id() -> io::Result<String> {
    platform::boot_id()
}

/// When process `pid` started, in the OS's own units, or `None` when it does not run.
pub fn start_time(pid: u32) -> Option<u64> {
    platform::start_time(pid)
}

#[cfg(target_os = "linux")]
mod platform {
    use std::io;

    pub fn boot_id() -> io::Result<String> {
        Ok(std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?
            .trim()
            .to_owned())
    }

    /// Field 22 of `/proc/<pid>/stat`: clock ticks from boot to the start.
    pub fn start_time(pid: u32) -> Option<u64> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // The name in field 2 may hold spaces and parentheses; fields after it do not.
        let after_name = &stat[stat.rfind(')')? + 1..];
        after_name.split_whitespace().nth(19)?.parse().ok()
    }
}

/// Written against `sysctl(3)` and `proc_pidinfo` as documented; not yet run on a Mac.
#[cfg(target_os = "macos")]
mod platform {
    use std::{io, ptr};

    pub fn boot_id() -> io::Result<String> {
        let mut boot = libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        let mut size = size_of::<libc::timeval>();
        // SAFETY: `boot` is a timeval and `size` says so.
        let result = unsafe {
            libc::sysctlbyname(
                c"kern.boottime".as_ptr(),
                (&raw mut boot).cast(),
                &mut size,
                ptr::null_mut(),
                0,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(format!("{}.{}", boot.tv_sec, boot.tv_usec))
    }

    /// Microseconds from the epoch to the start.
    pub fn start_time(pid: u32) -> Option<u64> {
        // SAFETY: proc_bsdinfo is plain data.
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = size_of::<libc::proc_bsdinfo>() as i32;
        // SAFETY: `info` is `size` bytes.
        let written = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                (&raw mut info).cast(),
                size,
            )
        };
        (written == size)
            .then(|| info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
    }
}

#[cfg(windows)]
mod platform {
    use std::io;

    use windows_sys::Win32::{
        Foundation::{FILETIME, STILL_ACTIVE},
        System::{
            Registry::HKEY_LOCAL_MACHINE,
            Threading::{
                GetExitCodeProcess, GetProcessTimes, OpenProcess,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };

    use crate::daemon::win::{Handle, registry_dword};

    /// Windows counts boots in the registry.
    pub fn boot_id() -> io::Result<String> {
        registry_dword(
            HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters",
            "BootId",
        )
        .map(|id| id.to_string())
    }

    /// 100-nanosecond intervals from 1601 to the start.
    pub fn start_time(pid: u32) -> Option<u64> {
        // SAFETY: the handle is owned by `Handle`.
        let process = Handle(unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)
        });
        if process.0.is_null() {
            return None;
        }
        let mut code = 0u32;
        // SAFETY: a valid process handle.
        if unsafe { GetExitCodeProcess(process.0, &raw mut code) } == 0
            || code != STILL_ACTIVE as u32
        {
            return None;
        }
        let mut times = [FILETIME::default(); 4];
        let [created, exited, kernel, user] = &mut times;
        // SAFETY: a valid process handle and four FILETIMEs.
        if unsafe { GetProcessTimes(process.0, created, exited, kernel, user) }
            == 0
        {
            return None;
        }
        Some(
            (u64::from(times[0].dwHighDateTime) << 32)
                | u64::from(times[0].dwLowDateTime),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_has_a_stable_identity() {
        let pid = std::process::id();
        let first = start_time(pid).expect("this process runs");
        assert_eq!(start_time(pid), Some(first));
        assert_eq!(boot_id().unwrap(), boot_id().unwrap());
    }
}
