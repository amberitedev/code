//! Starting Java as `copal-servers`, the shim's own child.
//!
//! Linux and macOS: through `sudo` and the one helper the sudoers file allows (see
//! `daemon::install::HELPER`). The helper prints its pid before it becomes Java, because the
//! shim's child is `sudo`, not Java.
//! Windows: `LogonUser` with a batch logon, then `CreateProcessAsUser`; `CreateProcessWithLogonW`
//! when the shim lacks the privileges for that. Java runs in a job object per server that caps
//! its memory and is not killed when the shim's handle closes.

use std::io::{Read, Write};

#[cfg(windows)]
use super::Launch;

/// Whose account Java runs as.
pub enum RunAs {
    /// Development without `copal install`: the shim's own account.
    Shim,
    /// `copal-servers`. On Windows the shim logs on with the password install stored.
    Servers {
        #[cfg(windows)]
        password: String,
    },
}

/// A running Java process and its standard streams.
pub struct Java {
    pub pid: u32,
    pub stdin: Box<dyn Write + Send>,
    pub stdout: Box<dyn Read + Send>,
    pub stderr: Box<dyn Read + Send>,
    pub killer: Killer,
    pub waiter: Waiter,
}

pub use platform::{Killer, Waiter, share_with_core, spawn};

#[cfg(unix)]
mod platform {
    use std::{
        io::{self, BufRead, BufReader},
        os::unix::process::ExitStatusExt,
        process::{Child, Command, Stdio},
    };

    use nix::{
        sys::signal::{Signal, kill},
        unistd::Pid,
    };

    use super::{Java, RunAs};
    use crate::{
        daemon::{accounts::SERVERS, install::HELPER},
        servers::shim::Launch,
    };

    /// `sudo` without a prompt, as `copal-servers`, through the helper.
    fn as_servers() -> Command {
        let mut command = Command::new("sudo");
        command.args(["-n", "-u", SERVERS, "--", HELPER]);
        command
    }

    /// Let Core read what a server wrote in `dir`: every file and folder `copal-servers` owns
    /// there gets group read, and folders group search. Minecraft writes `level.dat` as 0600,
    /// which Core, reading servers' files through the group, cannot back up otherwise. Only the
    /// owner may change a mode, so this runs as `copal-servers`; links are left alone. Nothing
    /// becomes readable that the server's own account could not already hand Core.
    pub fn share_with_core(dir: &std::path::Path) -> io::Result<()> {
        let output = share_command(dir).stdin(Stdio::null()).output()?;
        if output.status.success() {
            return Ok(());
        }
        // The helper's first line is its pid.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason: Vec<&str> = stderr
            .lines()
            .filter(|line| !line.starts_with("copal-pid "))
            .collect();
        Err(io::Error::other(format!(
            "could not let Core read {}: {}",
            dir.display(),
            reason.join("; ")
        )))
    }

    fn share_command(dir: &std::path::Path) -> Command {
        let mut command = as_servers();
        command
            .arg("find")
            .arg(dir)
            .args(["-user", SERVERS, "!", "-type", "l"])
            .args(["-exec", "chmod", "g+rX", "{}", "+"]);
        command
    }

    #[cfg(test)]
    #[test]
    fn sharing_changes_only_what_the_servers_account_owns_and_never_a_link() {
        let command =
            share_command(std::path::Path::new("/srv/copal/servers/a"));
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(command.get_program(), "sudo");
        assert_eq!(
            args,
            [
                "-n",
                "-u",
                SERVERS,
                "--",
                HELPER,
                "find",
                "/srv/copal/servers/a",
                "-user",
                SERVERS,
                "!",
                "-type",
                "l",
                "-exec",
                "chmod",
                "g+rX",
                "{}",
                "+",
            ]
        );
    }

    pub fn spawn(launch: &Launch, run_as: &RunAs) -> io::Result<Java> {
        let mut command = match run_as {
            RunAs::Shim => Command::new(&launch.java),
            RunAs::Servers {} => {
                let mut command = as_servers();
                command.arg(&launch.java);
                command
            }
        };
        let mut child = command
            .args(&launch.args)
            .current_dir(&launch.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let mut stderr = BufReader::new(child.stderr.take().expect("piped"));
        let pid = match run_as {
            RunAs::Shim => child.id(),
            RunAs::Servers {} => {
                let mut first = String::new();
                stderr.read_line(&mut first)?;
                match first.trim().strip_prefix("copal-pid ").map(str::parse) {
                    Some(Ok(pid)) => pid,
                    _ => {
                        let _ = child.wait();
                        return Err(io::Error::other(format!(
                            "could not start Java as {SERVERS}: {}",
                            first.trim()
                        )));
                    }
                }
            }
        };
        Ok(Java {
            pid,
            stdin: Box::new(stdin),
            stdout: Box::new(stdout),
            stderr: Box::new(stderr),
            killer: Killer {
                pid,
                as_servers: matches!(run_as, RunAs::Servers {}),
            },
            waiter: Waiter(child),
        })
    }

    /// Kills Java. `copal` may not signal a `copal-servers` process, so that goes through the
    /// helper too.
    pub struct Killer {
        pid: u32,
        as_servers: bool,
    }

    impl Killer {
        pub fn kill(&self) -> io::Result<()> {
            if self.as_servers {
                let status = as_servers()
                    .args(["kill", "-KILL", &self.pid.to_string()])
                    .stderr(Stdio::null())
                    .status()?;
                if !status.success() {
                    return Err(io::Error::other("kill failed"));
                }
                Ok(())
            } else {
                kill(Pid::from_raw(self.pid as i32), Signal::SIGKILL)
                    .map_err(io::Error::from)
            }
        }
    }

    pub struct Waiter(Child);

    impl Waiter {
        /// Wait for Java to exit. A process ended by a signal reports 128 plus the signal, like
        /// a shell.
        pub fn wait(mut self) -> io::Result<i32> {
            let status = self.0.wait()?;
            Ok(status
                .code()
                .or_else(|| status.signal().map(|signal| 128 + signal))
                .unwrap_or(-1))
        }
    }
}

#[cfg(windows)]
mod platform {
    /// Nothing to do on Windows: what a server writes inherits its folder's permissions, which
    /// let Core read it.
    pub fn share_with_core(_dir: &Path) -> io::Result<()> {
        Ok(())
    }

    use std::{
        fs::File, io, os::windows::io::FromRawHandle, path::Path, ptr,
        sync::Arc,
    };

    use windows_sys::Win32::{
        Foundation::{
            ERROR_PRIVILEGE_NOT_HELD, HANDLE, HANDLE_FLAG_INHERIT,
            INVALID_HANDLE_VALUE, SetHandleInformation,
        },
        Security::{LOGON32_LOGON_BATCH, LOGON32_PROVIDER_DEFAULT, LogonUserW},
        System::Console::{
            GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        },
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW,
                JOB_OBJECT_LIMIT_JOB_MEMORY,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectExtendedLimitInformation, SetInformationJobObject,
                TerminateJobObject,
            },
            Pipes::CreatePipe,
            Threading::{
                CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
                CreateProcessAsUserW, CreateProcessW, CreateProcessWithLogonW,
                GetExitCodeProcess, INFINITE, PROCESS_INFORMATION,
                ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOW,
                TerminateProcess, WaitForSingleObject,
            },
        },
    };

    use super::{Java, Launch, RunAs, command_line};
    use crate::daemon::{
        accounts::SERVERS,
        win::{Handle, check, enable_privileges, wide},
    };

    pub fn spawn(launch: &Launch, run_as: &RunAs) -> io::Result<Java> {
        keep_std_handles_private();
        let (stdin_child, stdin) = pipe()?;
        let (stdout, stdout_child) = pipe()?;
        let (stderr, stderr_child) = pipe()?;
        for child_end in [&stdin_child, &stdout_child, &stderr_child] {
            // SAFETY: a valid handle; only the child's ends are inherited.
            check(unsafe {
                SetHandleInformation(
                    child_end.0,
                    HANDLE_FLAG_INHERIT,
                    HANDLE_FLAG_INHERIT,
                )
            })?;
        }
        let job = Arc::new(job(launch.memory_limit_mb)?);

        let mut command =
            wide(command_line(&launch.java.to_string_lossy(), &launch.args));
        let environment = environment(&launch.dir);
        let dir = wide(&launch.dir);
        // An empty desktop name gives the new logon session a window station of its own, which
        // its token can use. Java does not start in one it may not open.
        let mut desktop = wide("");
        let mut startup = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            dwFlags: STARTF_USESTDHANDLES,
            hStdInput: stdin_child.0,
            hStdOutput: stdout_child.0,
            hStdError: stderr_child.0,
            ..Default::default()
        };
        let flags =
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW;
        let mut info = PROCESS_INFORMATION::default();

        match run_as {
            // SAFETY: every pointer is valid for the call; `command` is writable as required.
            RunAs::Shim => check(unsafe {
                CreateProcessW(
                    ptr::null(),
                    command.as_mut_ptr(),
                    ptr::null(),
                    ptr::null(),
                    1,
                    flags,
                    environment.as_ptr().cast(),
                    dir.as_ptr(),
                    &raw const startup,
                    &raw mut info,
                )
            })?,
            RunAs::Servers { password } => {
                startup.lpDesktop = desktop.as_mut_ptr();
                let user = wide(SERVERS);
                let domain = wide(".");
                let password = wide(password);
                let as_user =
                    logon(&user, &domain, &password).and_then(|token| {
                        enable_privileges(&[
                            "SeAssignPrimaryTokenPrivilege",
                            "SeIncreaseQuotaPrivilege",
                        ])?;
                        // SAFETY: as above, with a primary token for `copal-servers`.
                        check(unsafe {
                            CreateProcessAsUserW(
                                token.0,
                                ptr::null(),
                                command.as_mut_ptr(),
                                ptr::null(),
                                ptr::null(),
                                1,
                                flags,
                                environment.as_ptr().cast(),
                                dir.as_ptr(),
                                &raw const startup,
                                &raw mut info,
                            )
                        })
                    });
                match as_user {
                    Err(error)
                        if error.raw_os_error()
                            == Some(ERROR_PRIVILEGE_NOT_HELD as i32) =>
                    {
                        eprintln!(
                            "CreateProcessAsUser lacks a privilege ({error}); trying CreateProcessWithLogonW"
                        );
                        // SAFETY: as above.
                        check(unsafe {
                            CreateProcessWithLogonW(
                                user.as_ptr(),
                                domain.as_ptr(),
                                password.as_ptr(),
                                0,
                                ptr::null(),
                                command.as_mut_ptr(),
                                flags,
                                environment.as_ptr().cast(),
                                dir.as_ptr(),
                                &raw const startup,
                                &raw mut info,
                            )
                        })?;
                    }
                    result => result?,
                }
            }
        }
        let process = Arc::new(Handle(info.hProcess));
        let thread = Handle(info.hThread);
        // SAFETY: valid job and process handles.
        if let Err(error) =
            check(unsafe { AssignProcessToJobObject(job.0, process.0) })
        {
            // SAFETY: the suspended process never ran.
            unsafe { TerminateProcess(process.0, 1) };
            return Err(error);
        }
        // SAFETY: the main thread of the suspended process.
        if unsafe { ResumeThread(thread.0) } == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        Ok(Java {
            pid: info.dwProcessId,
            stdin: Box::new(file(stdin)),
            stdout: Box::new(file(stdout)),
            stderr: Box::new(file(stderr)),
            killer: Killer(job),
            waiter: Waiter(process),
        })
    }

    /// Java inherits every inheritable handle of the shim. The shim's standard handles came from
    /// Core, and its stderr is `shim.log`, which Java must not be able to write. Every other
    /// handle the shim opens is not inheritable, so Java gets only its three pipe ends.
    pub(super) fn keep_std_handles_private() {
        for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: GetStdHandle returns a handle owned by the process, or none.
            let handle = unsafe { GetStdHandle(id) };
            if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
                // SAFETY: a valid handle; only its inherit flag changes.
                unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
            }
        }
    }

    /// An anonymous pipe: (read end, write end). Neither end is inherited until told.
    fn pipe() -> io::Result<(Handle, Handle)> {
        let (mut read, mut write) = (ptr::null_mut(), ptr::null_mut());
        // SAFETY: both handles are owned by `Handle` below.
        check(unsafe {
            CreatePipe(&raw mut read, &raw mut write, ptr::null(), 0)
        })?;
        Ok((Handle(read), Handle(write)))
    }

    fn file(handle: Handle) -> File {
        // SAFETY: the handle is owned and moved into the file.
        unsafe { File::from_raw_handle(handle.into_raw()) }
    }

    fn logon(
        user: &[u16],
        domain: &[u16],
        password: &[u16],
    ) -> io::Result<Handle> {
        let mut token: HANDLE = ptr::null_mut();
        // SAFETY: NUL-terminated strings; the token is owned by `Handle`.
        check(unsafe {
            LogonUserW(
                user.as_ptr(),
                domain.as_ptr(),
                password.as_ptr(),
                LOGON32_LOGON_BATCH,
                LOGON32_PROVIDER_DEFAULT,
                &raw mut token,
            )
        })?;
        Ok(Handle(token))
    }

    /// A job that caps the memory of everything in it. Not killed when its last handle closes,
    /// so the server outlives a shim that dies.
    fn job(memory_limit_mb: u64) -> io::Result<Handle> {
        // SAFETY: an unnamed job with default security.
        let job = Handle(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) });
        if job.0.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_JOB_MEMORY;
        limits.JobMemoryLimit = (memory_limit_mb * 1024 * 1024) as usize;
        // SAFETY: `limits` is the struct the class names.
        check(unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        })?;
        Ok(job)
    }

    /// The few variables Java needs. An account without a loaded profile has no usable
    /// `%TEMP%`, so Java's temp folder is inside the server folder.
    fn environment(dir: &Path) -> Vec<u16> {
        let system_root = std::env::var("SystemRoot")
            .unwrap_or_else(|_| r"C:\Windows".into());
        let temp = dir.join("tmp").to_string_lossy().into_owned();
        let mut variables = [
            ("PATH", format!(r"{system_root}\System32;{system_root}")),
            ("SystemRoot", system_root.clone()),
            ("TEMP", temp.clone()),
            ("TMP", temp),
            ("windir", system_root),
        ];
        variables.sort_by_key(|(name, _)| name.to_ascii_uppercase());
        let mut block: Vec<u16> = variables
            .iter()
            .flat_map(|(name, value)| wide(format!("{name}={value}")))
            .collect();
        block.push(0);
        block
    }

    /// Kills everything in the server's job.
    pub struct Killer(Arc<Handle>);

    impl Killer {
        pub fn kill(&self) -> io::Result<()> {
            // SAFETY: a valid job handle.
            check(unsafe { TerminateJobObject(self.0.0, 1) })
        }
    }

    pub struct Waiter(Arc<Handle>);

    impl Waiter {
        pub fn wait(self) -> io::Result<i32> {
            // SAFETY: a valid process handle.
            unsafe { WaitForSingleObject(self.0.0, INFINITE) };
            let mut code = 0u32;
            // SAFETY: a valid process handle.
            check(unsafe { GetExitCodeProcess(self.0.0, &raw mut code) })?;
            Ok(code as i32)
        }
    }
}

/// A Windows command line that hands each argument to the program unchanged, quoted the way
/// the C runtime splits it.
#[cfg_attr(not(windows), allow(dead_code))]
fn command_line(program: &str, args: &[String]) -> String {
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .map(quote)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg_attr(not(windows), allow(dead_code))]
fn quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\x0b', '"']) {
        return arg.to_owned();
    }
    let mut quoted = String::from('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            c => {
                quoted.push_str(&"\\".repeat(backslashes));
                quoted.push(c);
                backslashes = 0;
            }
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn java_cannot_inherit_the_shims_own_handles() {
        use windows_sys::Win32::{
            Foundation::{
                GetHandleInformation, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
            },
            System::Console::{
                GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
                STD_OUTPUT_HANDLE,
            },
        };

        platform::keep_std_handles_private();
        for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: reads this process's standard handles and their flags.
            unsafe {
                let handle = GetStdHandle(id);
                let mut flags = 0;
                if !handle.is_null()
                    && handle != INVALID_HANDLE_VALUE
                    && GetHandleInformation(handle, &raw mut flags) != 0
                {
                    assert_eq!(flags & HANDLE_FLAG_INHERIT, 0);
                }
            }
        }
    }

    #[test]
    fn arguments_are_quoted_for_the_c_runtime() {
        assert_eq!(
            command_line(
                r"C:\Java\bin\java.exe",
                &[
                    "-Xmx2G".into(),
                    r"C:\with space\".into(),
                    r#"a"b"#.into(),
                    String::new()
                ]
            ),
            r#"C:\Java\bin\java.exe -Xmx2G "C:\with space\\" "a\"b" """#
        );
    }
}
