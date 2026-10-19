//! The two OS accounts `copal install` creates. Core and its shims run as `copal`; Java and
//! anything else that runs a server's code runs as `copal-servers`, which reaches only the
//! folders `paths::FOLDERS` grants it. Neither can log on interactively.

use color_eyre::eyre::{Result, WrapErr};

pub const CORE: &str = "copal";
pub const SERVERS: &str = "copal-servers";

/// Both accounts, by their OS ids.
#[derive(Debug, Clone)]
pub struct Accounts {
    pub core: Id,
    pub servers: Id,
}

/// An account's OS id: a uid and its primary group on Linux and macOS.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Id {
    pub uid: u32,
    pub gid: u32,
}

/// An account's OS id: a SID on Windows.
#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Id {
    pub sid: String,
}

/// What `create` made. Windows also returns the passwords: `copal`'s goes to the service
/// manager only, `copal-servers`' to the install record so the shim can log on as it.
pub struct Created {
    pub accounts: Accounts,
    #[cfg(windows)]
    pub core_password: String,
    #[cfg(windows)]
    pub servers_password: String,
}

impl Accounts {
    /// Both accounts, or `None` when Copal is not installed on this machine.
    pub fn find() -> Result<Option<Self>> {
        platform::find()
    }

    /// Whether this process runs as `copal`.
    pub fn is_current(&self) -> Result<bool> {
        platform::is_current(&self.core)
    }
}

/// Create both accounts, or bring existing ones up to date. Needs admin.
pub fn create() -> Result<Created> {
    platform::create().wrap_err("could not create the Copal accounts")
}

/// Delete both accounts. Accounts that are already gone are skipped. Needs admin.
pub fn remove() -> Result<()> {
    platform::remove().wrap_err("could not remove the Copal accounts")
}

/// The OS id of the account running this process, as the install record stores it.
pub fn current_id() -> Result<String> {
    platform::current_id()
}

/// The OS id of a named account, as the install record stores it.
pub fn id_of(name: &str) -> Result<String> {
    platform::id_of(name)
}

#[cfg(unix)]
mod unix {
    use std::process::Command;

    use color_eyre::eyre::{Result, bail, eyre};
    use nix::unistd::{Group, User, getuid};

    use super::{Accounts, CORE, Id, SERVERS};

    pub fn find() -> Result<Option<Accounts>> {
        let (Some(core), Some(servers), Some(group)) = (
            User::from_name(CORE)?,
            User::from_name(SERVERS)?,
            Group::from_name(SERVERS)?,
        ) else {
            return Ok(None);
        };
        Ok(Some(Accounts {
            core: Id {
                uid: core.uid.as_raw(),
                gid: core.gid.as_raw(),
            },
            servers: Id {
                uid: servers.uid.as_raw(),
                gid: group.gid.as_raw(),
            },
        }))
    }

    pub fn is_current(core: &Id) -> Result<bool> {
        Ok(nix::unistd::geteuid().as_raw() == core.uid)
    }

    /// `sudo` sets `SUDO_UID` to the account that ran it, which is the one meant.
    pub fn current_id() -> Result<String> {
        Ok(std::env::var("SUDO_UID")
            .ok()
            .filter(|uid| uid.parse::<u32>().is_ok())
            .unwrap_or_else(|| getuid().to_string()))
    }

    pub fn id_of(name: &str) -> Result<String> {
        let user =
            User::from_name(name)?.ok_or_else(|| eyre!("no account {name}"))?;
        Ok(user.uid.to_string())
    }

    /// Run a program that must succeed.
    pub fn run(program: &str, args: &[&str]) -> Result<()> {
        let output = Command::new(program).args(args).output()?;
        if !output.status.success() {
            bail!(
                "{program} {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use color_eyre::eyre::{Result, eyre};
    use nix::unistd::{Group, User};

    pub use super::unix::{current_id, find, id_of, is_current};
    use super::{CORE, Created, SERVERS, unix::run};

    fn nologin() -> &'static str {
        ["/usr/sbin/nologin", "/sbin/nologin"]
            .into_iter()
            .find(|path| std::path::Path::new(path).exists())
            .unwrap_or("/bin/false")
    }

    pub fn create() -> Result<Created> {
        for name in [SERVERS, CORE] {
            if User::from_name(name)?.is_none() {
                run(
                    "useradd",
                    &[
                        "--system",
                        "--user-group",
                        "--no-create-home",
                        "--home-dir",
                        "/nonexistent",
                        "--shell",
                        nologin(),
                        name,
                    ],
                )?;
            }
        }
        // `copal` reads and cleans up the files servers write through this group.
        run("usermod", &["--append", "--groups", SERVERS, CORE])?;
        let accounts =
            find()?.ok_or_else(|| eyre!("the accounts were not created"))?;
        Ok(Created { accounts })
    }

    pub fn remove() -> Result<()> {
        for name in [CORE, SERVERS] {
            if User::from_name(name)?.is_some() {
                run("userdel", &[name])?;
            }
            if Group::from_name(name)?.is_some() {
                run("groupdel", &[name])?;
            }
        }
        Ok(())
    }
}

/// Written against `dscl` and `dseditgroup` as documented; not yet run on a Mac.
#[cfg(target_os = "macos")]
mod platform {
    use std::process::Command;

    use color_eyre::eyre::{Result, eyre};
    use nix::unistd::{Group, User};

    pub use super::unix::{current_id, find, id_of, is_current};
    use super::{CORE, Created, SERVERS, unix::run};

    /// Ids below 500 are hidden system accounts on macOS.
    fn free_id() -> Result<u32> {
        let mut used = Vec::new();
        for (kind, key) in
            [("/Users", "UniqueID"), ("/Groups", "PrimaryGroupID")]
        {
            let output = Command::new("dscl")
                .args([".", "-list", kind, key])
                .output()?;
            used.extend(
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter_map(|line| line.split_whitespace().last())
                    .filter_map(|id| id.parse::<u32>().ok()),
            );
        }
        (400..500)
            .find(|id| !used.contains(id))
            .ok_or_else(|| eyre!("no free account id between 400 and 499"))
    }

    pub fn create() -> Result<Created> {
        for name in [SERVERS, CORE] {
            if User::from_name(name)?.is_some() {
                continue;
            }
            let id = free_id()?.to_string();
            let group = format!("/Groups/{name}");
            let user = format!("/Users/{name}");
            run("dscl", &[".", "-create", &group])?;
            run("dscl", &[".", "-create", &group, "PrimaryGroupID", &id])?;
            run("dscl", &[".", "-create", &group, "Password", "*"])?;
            run("dscl", &[".", "-create", &user])?;
            for (key, value) in [
                ("UniqueID", id.as_str()),
                ("PrimaryGroupID", id.as_str()),
                ("UserShell", "/usr/bin/false"),
                ("NFSHomeDirectory", "/var/empty"),
                ("RealName", name),
                ("Password", "*"),
                ("IsHidden", "1"),
            ] {
                run("dscl", &[".", "-create", &user, key, value])?;
            }
        }
        run(
            "dseditgroup",
            &["-o", "edit", "-a", CORE, "-t", "user", SERVERS],
        )?;
        let accounts =
            find()?.ok_or_else(|| eyre!("the accounts were not created"))?;
        Ok(Created { accounts })
    }

    pub fn remove() -> Result<()> {
        for name in [CORE, SERVERS] {
            if User::from_name(name)?.is_some() {
                run("dscl", &[".", "-delete", &format!("/Users/{name}")])?;
            }
            if Group::from_name(name)?.is_some() {
                run("dscl", &[".", "-delete", &format!("/Groups/{name}")])?;
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
mod platform {
    use std::{io, ptr};

    use color_eyre::eyre::{Result, WrapErr};
    use windows_sys::Win32::{
        Foundation::ERROR_NONE_MAPPED,
        NetworkManagement::NetManagement::{
            NERR_UserExists, NERR_UserNotFound, NetUserAdd, NetUserDel,
            NetUserSetInfo, UF_DONT_EXPIRE_PASSWD, UF_PASSWD_CANT_CHANGE,
            UF_SCRIPT, USER_INFO_1, USER_INFO_1003, USER_PRIV_USER,
        },
        Security::{
            Authentication::Identity::{
                LSA_HANDLE, LSA_OBJECT_ATTRIBUTES, LSA_UNICODE_STRING,
                LsaAddAccountRights, LsaClose, LsaNtStatusToWinError,
                LsaOpenPolicy, LsaRemoveAccountRights, POLICY_CREATE_ACCOUNT,
                POLICY_LOOKUP_NAMES,
            },
            PSID,
        },
        System::Registry::HKEY_LOCAL_MACHINE,
        UI::Shell::DeleteProfileW,
    };

    use super::{Accounts, CORE, Created, Id, SERVERS};
    use crate::daemon::win::{
        account_sid, check_code, current_sid, delete_registry_value,
        set_registry_dword, wide,
    };

    /// Accounts listed here with 0 are left off the sign-in screen.
    const HIDDEN: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList";
    /// Neither account may sign in at the keyboard or over Remote Desktop.
    const DENY_LOGON: [&str; 2] = [
        "SeDenyInteractiveLogonRight",
        "SeDenyRemoteInteractiveLogonRight",
    ];
    /// `copal` runs the service and starts Java as `copal-servers` with
    /// `CreateProcessAsUser`, which needs the two privileges.
    const CORE_RIGHTS: [&str; 3] = [
        "SeServiceLogonRight",
        "SeAssignPrimaryTokenPrivilege",
        "SeIncreaseQuotaPrivilege",
    ];
    /// The shim logs on as `copal-servers` with a batch logon.
    const SERVERS_RIGHTS: [&str; 1] = ["SeBatchLogonRight"];

    pub fn find() -> Result<Option<Accounts>> {
        let sid = |name| match account_sid(name) {
            Ok(sid) => Ok(Some(Id { sid })),
            Err(error)
                if error.raw_os_error() == Some(ERROR_NONE_MAPPED as i32) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        };
        Ok(match (sid(CORE)?, sid(SERVERS)?) {
            (Some(core), Some(servers)) => Some(Accounts { core, servers }),
            _ => None,
        })
    }

    pub fn is_current(core: &Id) -> Result<bool> {
        Ok(current_sid()? == core.sid)
    }

    /// An elevated process runs as the same account as the one that elevated it.
    pub fn current_id() -> Result<String> {
        Ok(current_sid()?)
    }

    pub fn id_of(name: &str) -> Result<String> {
        account_sid(name).wrap_err_with(|| format!("no account {name}"))
    }

    /// 64 random hex digits plus one upper case letter and one symbol, so every Windows
    /// password policy accepts it.
    fn password() -> String {
        format!(
            "C!{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        )
    }

    pub fn create() -> Result<Created> {
        let core_password = password();
        let servers_password = password();
        user(
            CORE,
            &core_password,
            "Runs Copal, the Minecraft server manager",
        )?;
        user(
            SERVERS,
            &servers_password,
            "Runs the Minecraft servers Copal manages",
        )?;
        let accounts = find()?.ok_or_else(|| {
            color_eyre::eyre::eyre!("the accounts were not created")
        })?;
        let policy = Policy::open()?;
        policy.add(
            &accounts.core.sid,
            &[&CORE_RIGHTS[..], &DENY_LOGON].concat(),
        )?;
        policy.add(
            &accounts.servers.sid,
            &[&SERVERS_RIGHTS[..], &DENY_LOGON].concat(),
        )?;
        for name in [CORE, SERVERS] {
            set_registry_dword(HKEY_LOCAL_MACHINE, HIDDEN, name, 0)?;
        }
        Ok(Created {
            accounts,
            core_password,
            servers_password,
        })
    }

    /// Create a local account, or reset the password of one that exists.
    fn user(name: &str, password: &str, comment: &str) -> io::Result<()> {
        let mut name_w = wide(name);
        let mut password_w = wide(password);
        let mut comment_w = wide(comment);
        let info = USER_INFO_1 {
            usri1_name: name_w.as_mut_ptr(),
            usri1_password: password_w.as_mut_ptr(),
            usri1_password_age: 0,
            usri1_priv: USER_PRIV_USER,
            usri1_home_dir: ptr::null_mut(),
            usri1_comment: comment_w.as_mut_ptr(),
            usri1_flags: UF_SCRIPT
                | UF_DONT_EXPIRE_PASSWD
                | UF_PASSWD_CANT_CHANGE,
            usri1_script_path: ptr::null_mut(),
        };
        // SAFETY: `info` and its strings live across the call.
        let code = unsafe {
            NetUserAdd(
                ptr::null(),
                1,
                (&raw const info).cast(),
                ptr::null_mut(),
            )
        };
        if code != NERR_UserExists {
            return check_code(code);
        }
        let info = USER_INFO_1003 {
            usri1003_password: password_w.as_mut_ptr(),
        };
        // SAFETY: `info` lives across the call.
        check_code(unsafe {
            NetUserSetInfo(
                ptr::null(),
                name_w.as_ptr(),
                1003,
                (&raw const info).cast(),
                ptr::null_mut(),
            )
        })
    }

    pub fn remove() -> Result<()> {
        if let Some(accounts) = find()? {
            let policy = Policy::open()?;
            for id in [&accounts.core, &accounts.servers] {
                policy.remove_all(&id.sid)?;
                // The service's profile folder; most accounts never had one.
                // SAFETY: a valid SID string.
                unsafe {
                    DeleteProfileW(
                        wide(&id.sid).as_ptr(),
                        ptr::null(),
                        ptr::null(),
                    )
                };
            }
        }
        for name in [CORE, SERVERS] {
            // SAFETY: a valid account name.
            let code = unsafe { NetUserDel(ptr::null(), wide(name).as_ptr()) };
            if code != NERR_UserNotFound {
                check_code(code)?;
            }
            delete_registry_value(HKEY_LOCAL_MACHINE, HIDDEN, name)?;
        }
        Ok(())
    }

    /// The local security policy, for account rights.
    struct Policy(LSA_HANDLE);

    impl Policy {
        fn open() -> io::Result<Self> {
            let attributes = LSA_OBJECT_ATTRIBUTES::default();
            let mut handle = 0;
            // SAFETY: zeroed attributes are what LsaOpenPolicy expects.
            lsa(unsafe {
                LsaOpenPolicy(
                    ptr::null(),
                    &raw const attributes,
                    (POLICY_CREATE_ACCOUNT | POLICY_LOOKUP_NAMES) as u32,
                    &raw mut handle,
                )
            })?;
            Ok(Self(handle))
        }

        fn add(&self, sid: &str, rights: &[&str]) -> io::Result<()> {
            let sid = Sid::parse(sid)?;
            let mut names: Vec<Vec<u16>> = rights.iter().map(wide).collect();
            let strings: Vec<LSA_UNICODE_STRING> =
                names.iter_mut().map(|name| lsa_string(name)).collect();
            // SAFETY: the strings and SID live across the call.
            lsa(unsafe {
                LsaAddAccountRights(
                    self.0,
                    sid.0,
                    strings.as_ptr(),
                    strings.len() as u32,
                )
            })
        }

        fn remove_all(&self, sid: &str) -> io::Result<()> {
            let sid = Sid::parse(sid)?;
            // SAFETY: a valid SID; `true` removes every right.
            let result = lsa(unsafe {
                LsaRemoveAccountRights(self.0, sid.0, true, ptr::null(), 0)
            });
            match result {
                // An account with no rights left has no policy entry.
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                result => result,
            }
        }
    }

    impl Drop for Policy {
        fn drop(&mut self) {
            // SAFETY: opened by LsaOpenPolicy.
            unsafe { LsaClose(self.0) };
        }
    }

    fn lsa(status: i32) -> io::Result<()> {
        // SAFETY: converts a status code.
        check_code(unsafe { LsaNtStatusToWinError(status) })
    }

    /// `name` without its terminating NUL, which LSA strings leave out.
    fn lsa_string(name: &mut [u16]) -> LSA_UNICODE_STRING {
        let bytes = ((name.len() - 1) * 2) as u16;
        LSA_UNICODE_STRING {
            Length: bytes,
            MaximumLength: bytes + 2,
            Buffer: name.as_mut_ptr(),
        }
    }

    /// A binary SID parsed from its string form, freed on drop.
    struct Sid(PSID);

    impl Sid {
        fn parse(text: &str) -> io::Result<Self> {
            use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
            let mut sid = ptr::null_mut();
            // SAFETY: freed by Drop.
            crate::daemon::win::check(unsafe {
                ConvertStringSidToSidW(wide(text).as_ptr(), &raw mut sid)
            })?;
            Ok(Self(sid))
        }
    }

    impl Drop for Sid {
        fn drop(&mut self) {
            // SAFETY: allocated by ConvertStringSidToSidW with LocalAlloc.
            unsafe { windows_sys::Win32::Foundation::LocalFree(self.0) };
        }
    }
}
