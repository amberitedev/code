//! Small wrappers over the Win32 calls more than one module needs: wide strings, handles, account
//! ids (SIDs), security descriptors written as SDDL, privileges, and the registry.

use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, path::Path, ptr};

use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_NOT_ALL_ASSIGNED, GetLastError, HANDLE, LUID,
        LocalFree,
    },
    Security::{
        AdjustTokenPrivileges,
        Authorization::{
            ConvertSidToStringSidW,
            ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1, SE_FILE_OBJECT, SetNamedSecurityInfoW,
        },
        DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl,
        GetTokenInformation, LUID_AND_ATTRIBUTES, LookupAccountNameW,
        LookupPrivilegeValueW, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, PSID, SE_PRIVILEGE_ENABLED, SECURITY_ATTRIBUTES,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    },
    System::{
        Registry::{
            HKEY, REG_DWORD, RRF_RT_REG_DWORD, RegDeleteKeyValueW,
            RegGetValueW, RegSetKeyValueW,
        },
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
    UI::Shell::IsUserAnAdmin,
};

/// A NUL-terminated UTF-16 copy of `text`, for Win32 calls.
pub fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain(Some(0)).collect()
}

/// `Ok` when a Win32 call that returns `BOOL` succeeded, else the thread's last error.
pub fn check(result: i32) -> io::Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// `Ok` when a Win32 call that returns an error code succeeded.
pub fn check_code(code: u32) -> io::Result<()> {
    match code {
        0 => Ok(()),
        code => Err(io::Error::from_raw_os_error(code as i32)),
    }
}

/// A kernel handle, closed on drop.
#[derive(Debug)]
pub struct Handle(pub HANDLE);

// SAFETY: kernel handles are process-wide and may be used from any thread.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Handle {
    /// Give up ownership without closing.
    pub fn into_raw(self) -> HANDLE {
        let handle = self.0;
        std::mem::forget(self);
        handle
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the handle is owned by this value and closed once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Whether this process runs with administrator rights.
pub fn is_admin() -> bool {
    // SAFETY: no arguments.
    unsafe { IsUserAnAdmin() != 0 }
}

/// The SID (`S-1-5-21-...`) of a local account.
pub fn account_sid(name: &str) -> io::Result<String> {
    let name = wide(name);
    let (mut sid_len, mut domain_len, mut kind) = (0u32, 0u32, 0);
    // SAFETY: a size query with null buffers; the lengths are written.
    unsafe {
        LookupAccountNameW(
            ptr::null(),
            name.as_ptr(),
            ptr::null_mut(),
            &raw mut sid_len,
            ptr::null_mut(),
            &raw mut domain_len,
            &raw mut kind,
        );
    }
    if sid_len == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut sid = vec![0usize; (sid_len as usize).div_ceil(size_of::<usize>())];
    let mut domain = vec![0u16; domain_len as usize];
    // SAFETY: both buffers are at least as large as the sizes passed.
    check(unsafe {
        LookupAccountNameW(
            ptr::null(),
            name.as_ptr(),
            sid.as_mut_ptr().cast(),
            &raw mut sid_len,
            domain.as_mut_ptr(),
            &raw mut domain_len,
            &raw mut kind,
        )
    })?;
    sid_string(sid.as_mut_ptr().cast())
}

/// The SID of the account this process runs as.
pub fn current_sid() -> io::Result<String> {
    let token = process_token(TOKEN_QUERY)?;
    let mut len = 0u32;
    // SAFETY: a size query with a null buffer.
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            ptr::null_mut(),
            0,
            &raw mut len,
        )
    };
    let mut buffer = vec![0usize; (len as usize).div_ceil(size_of::<usize>())];
    // SAFETY: the buffer is `len` bytes, aligned for TOKEN_USER.
    check(unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            len,
            &raw mut len,
        )
    })?;
    // SAFETY: GetTokenInformation filled the buffer with a TOKEN_USER.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    sid_string(user.User.Sid)
}

fn sid_string(sid: PSID) -> io::Result<String> {
    let mut text = ptr::null_mut();
    // SAFETY: `sid` is a valid SID; the string is freed below.
    check(unsafe { ConvertSidToStringSidW(sid, &raw mut text) })?;
    // SAFETY: ConvertSidToStringSidW returns a NUL-terminated string.
    let string = unsafe { from_wide_ptr(text) };
    // SAFETY: allocated by ConvertSidToStringSidW with LocalAlloc.
    unsafe { LocalFree(text.cast()) };
    Ok(string)
}

/// A NUL-terminated UTF-16 string as a `String`.
///
/// # Safety
/// `text` must point to a NUL-terminated UTF-16 string.
unsafe fn from_wide_ptr(text: *const u16) -> String {
    let mut len = 0;
    // SAFETY: the caller promises a terminating NUL.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` units were just read.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) })
}

fn process_token(access: u32) -> io::Result<Handle> {
    let mut token = ptr::null_mut();
    // SAFETY: the pseudo handle needs no closing; the token is owned by `Handle`.
    check(unsafe {
        OpenProcessToken(GetCurrentProcess(), access, &raw mut token)
    })?;
    Ok(Handle(token))
}

/// A security descriptor parsed from SDDL, freed on drop.
pub struct SecurityDescriptor(PSECURITY_DESCRIPTOR);

// SAFETY: the descriptor is plain memory owned by this value.
unsafe impl Send for SecurityDescriptor {}
unsafe impl Sync for SecurityDescriptor {}

impl SecurityDescriptor {
    pub fn from_sddl(sddl: &str) -> io::Result<Self> {
        let mut descriptor = ptr::null_mut();
        // SAFETY: the descriptor is freed by Drop.
        check(unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(sddl).as_ptr(),
                SDDL_REVISION_1,
                &raw mut descriptor,
                ptr::null_mut(),
            )
        })?;
        Ok(Self(descriptor))
    }

    pub fn as_ptr(&self) -> PSECURITY_DESCRIPTOR {
        self.0
    }

    /// Attributes that point at this descriptor. Valid while `self` lives.
    pub fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        }
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW.
        unsafe { LocalFree(self.0.cast()) };
    }
}

/// Replace the permissions of `path` with the DACL in `sddl`, ignoring the parent's, and pass
/// its inheritable entries down to everything already inside.
pub fn set_path_permissions(path: &Path, sddl: &str) -> io::Result<()> {
    let descriptor = SecurityDescriptor::from_sddl(sddl)?;
    let (mut present, mut defaulted) = (0, 0);
    let mut dacl = ptr::null_mut();
    // SAFETY: `dacl` points into `descriptor`, which outlives its use.
    check(unsafe {
        GetSecurityDescriptorDacl(
            descriptor.as_ptr(),
            &raw mut present,
            &raw mut dacl,
            &raw mut defaulted,
        )
    })?;
    // SAFETY: a valid path and DACL.
    check_code(unsafe {
        SetNamedSecurityInfoW(
            wide(path).as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            dacl,
            ptr::null(),
        )
    })
}

/// Turn on privileges this process already holds. `CreateProcessAsUser` needs "replace a
/// process level token" and "adjust memory quotas" turned on.
pub fn enable_privileges(names: &[&str]) -> io::Result<()> {
    let token = process_token(TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY)?;
    for name in names {
        let mut luid = LUID::default();
        // SAFETY: a valid privilege name.
        check(unsafe {
            LookupPrivilegeValueW(
                ptr::null(),
                wide(name).as_ptr(),
                &raw mut luid,
            )
        })?;
        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        // SAFETY: a valid token and privilege list.
        check(unsafe {
            AdjustTokenPrivileges(
                token.0,
                0,
                &raw const privileges,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        })?;
        // AdjustTokenPrivileges succeeds even when the token lacks the privilege.
        // SAFETY: reads the thread's last error.
        if unsafe { GetLastError() } == ERROR_NOT_ALL_ASSIGNED {
            return Err(io::Error::other(format!(
                "this account does not hold {name}"
            )));
        }
    }
    Ok(())
}

/// A `REG_DWORD` value.
pub fn registry_dword(key: HKEY, subkey: &str, value: &str) -> io::Result<u32> {
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `data` is a DWORD and `size` says so.
    check_code(unsafe {
        RegGetValueW(
            key,
            wide(subkey).as_ptr(),
            wide(value).as_ptr(),
            RRF_RT_REG_DWORD,
            ptr::null_mut(),
            (&raw mut data).cast(),
            &raw mut size,
        )
    })?;
    Ok(data)
}

/// Write a `REG_DWORD` value, creating the key when missing.
pub fn set_registry_dword(
    key: HKEY,
    subkey: &str,
    value: &str,
    data: u32,
) -> io::Result<()> {
    // SAFETY: `data` is a DWORD and the size says so.
    check_code(unsafe {
        RegSetKeyValueW(
            key,
            wide(subkey).as_ptr(),
            wide(value).as_ptr(),
            REG_DWORD,
            (&raw const data).cast(),
            size_of::<u32>() as u32,
        )
    })
}

/// Delete a registry value. A value that is already gone is not an error.
pub fn delete_registry_value(
    key: HKEY,
    subkey: &str,
    value: &str,
) -> io::Result<()> {
    // SAFETY: valid strings.
    let code = unsafe {
        RegDeleteKeyValueW(key, wide(subkey).as_ptr(), wide(value).as_ptr())
    };
    match check_code(code) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
