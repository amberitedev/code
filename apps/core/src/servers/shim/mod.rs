//! The shim: `copal shim`, this binary in another mode. Core starts one per server run, fully
//! detached, running as `copal`. The shim starts Java as `copal-servers` as its own child, writes
//! Java's output to `console.log`, passes Core's commands to Java's stdin over a socket only
//! `copal` can open, and writes how Java ended to `status.json`. It outlives Core, so a Core that
//! stops, crashes, or updates leaves the server running.
//!
//! Its files, in `runs/<server>/` (see `daemon::paths`):
//! - `lock`: held by the shim while it runs, so a server never runs twice.
//! - `launch.json`: what Core asks for, a [`Launch`].
//! - `run.json`: who the shim is, a [`RunRecord`], written once Java runs.
//! - `status.json`: how the run ended, an [`ExitStatus`].
//! - `console.log` and `shim.log`.
//!
//! The shim never opens a path inside `servers/`: a mod could plant a link there. It only hands
//! the server folder to Java as its working directory.
//!
//! Shims outlive Core, so after an update a new Core meets old shims and after a rollback an old
//! Core meets new ones. Every message and file carries [`PROTOCOL`]; Core leaves a shim with
//! another version running and shows "restart to upgrade".

pub mod java;
mod run;

use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub use run::main;

use crate::{
    access::local::{Stream, connect_to},
    daemon::paths::Paths,
};

/// The version of the files and messages below. Change it whenever their meaning changes.
pub const PROTOCOL: u32 = 1;

/// What Core asks a shim to run, in `launch.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Launch {
    pub v: u32,
    pub run_id: String,
    pub java: PathBuf,
    pub args: Vec<String>,
    /// The server folder. Java's working directory; the shim opens nothing in it.
    pub dir: PathBuf,
    /// Windows: the cap of the server's job object.
    pub memory_limit_mb: u64,
}

/// Who a running shim is, in `run.json`. Core trusts the pid only while the start time and boot
/// still match, because pids are reused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub v: u32,
    pub run_id: String,
    /// The shim's pid. The shim lives exactly as long as Java and is the process Core talks to.
    pub pid: u32,
    /// See `servers::process::start_time`.
    pub start_time: u64,
    /// See `servers::process::boot_id`.
    pub boot_id: String,
    pub java_pid: u32,
}

impl RunRecord {
    /// Whether this record still names a running process, given this boot's id and the start
    /// time of whatever process now has the recorded pid.
    pub fn is_alive(&self, boot_id: &str, start_time: Option<u64>) -> bool {
        self.boot_id == boot_id && start_time == Some(self.start_time)
    }
}

/// How a run ended, in `status.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitStatus {
    pub v: u32,
    pub run_id: String,
    /// `None` when Java never started.
    pub exit_code: Option<i32>,
    pub ended_at: i64,
    pub ended_by: EndedBy,
    /// Why Java could not start.
    pub error: Option<String>,
}

/// What made Java exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndedBy {
    /// Java exited by itself, including a `stop` typed into the console.
    Exit,
    /// Core asked for a stop.
    Stop,
    /// Core asked for a kill.
    Kill,
}

/// A message on a shim's socket, one JSON object per line, inside an [`Envelope`]. The shim
/// greets every connection with `Hello` and answers holds and releases; Core sends the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello {
        run_id: String,
    },
    /// A line for Java's stdin.
    Command {
        line: String,
    },
    /// Send `stop` to Java and remember that Core asked.
    Stop,
    /// Kill Java now.
    Kill,
    /// Send `save-off` for a backup. The shim sends `save-on` itself after `seconds`, at most
    /// [`MAX_SAVE_HOLD_S`], unless Core releases it first; a second hold starts the time again.
    HoldSaves {
        seconds: u64,
    },
    /// Send `save-on` now, ending a hold.
    ReleaseSaves,
    /// The shim's answer to `HoldSaves`: `save-off` is sent, and `save-on` follows after
    /// `seconds` unless released first.
    SavesHeld {
        seconds: u64,
    },
    /// The shim's answer to `ReleaseSaves`: saving is on.
    SavesReleased,
}

/// The longest autosave stays off for a backup, whatever Core asked.
pub const MAX_SAVE_HOLD_S: u64 = 600;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Envelope {
    v: u32,
    #[serde(flatten)]
    message: Message,
}

/// Every file and message starts with its version, read before anything else.
#[derive(Deserialize)]
struct Versioned {
    v: u32,
}

/// Why Core could not use a shim.
#[derive(Debug, thiserror::Error)]
pub enum ShimError {
    #[error("the server's shim is not running: {0}")]
    Gone(#[from] io::Error),
    #[error(
        "this server was started by another Copal version (shim protocol {0}); restart it to upgrade"
    )]
    Incompatible(u32),
    #[error("the shim sent something unreadable: {0}")]
    Invalid(String),
}

/// Encode a message as one line.
fn encode(message: Message) -> String {
    let mut line = serde_json::to_string(&Envelope {
        v: PROTOCOL,
        message,
    })
    .expect("messages serialize");
    line.push('\n');
    line
}

/// Decode one line. The inner `Err` is the version of a message from another protocol.
fn decode(line: &str) -> Result<Result<Message, u32>, ShimError> {
    let invalid =
        |error: serde_json::Error| ShimError::Invalid(error.to_string());
    let Versioned { v } = serde_json::from_str(line).map_err(invalid)?;
    if v != PROTOCOL {
        return Ok(Err(v));
    }
    let envelope: Envelope = serde_json::from_str(line).map_err(invalid)?;
    Ok(Ok(envelope.message))
}

/// Read a JSON file, checking its version first.
pub fn read_versioned<T: DeserializeOwned>(
    path: &Path,
) -> Result<T, ShimError> {
    let text = std::fs::read_to_string(path)?;
    let invalid =
        |error: serde_json::Error| ShimError::Invalid(error.to_string());
    let Versioned { v } = serde_json::from_str(&text).map_err(invalid)?;
    if v != PROTOCOL {
        return Err(ShimError::Incompatible(v));
    }
    serde_json::from_str(&text).map_err(invalid)
}

/// Write a JSON file whole: readers see the old file or the new one, never half of one.
pub fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let staged = path.with_extension("json.new");
    std::fs::write(&staged, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&staged, path)
}

/// Take a server's run lock, or `None` when a shim holds it. The lock is released when the
/// returned file is dropped or the process exits, however it exits.
pub fn try_lock(paths: &Paths, server_id: &str) -> io::Result<Option<File>> {
    std::fs::create_dir_all(paths.run(server_id))?;
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(paths.run_lock(server_id))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

/// Whether a shim runs for the server.
pub fn is_running(paths: &Paths, server_id: &str) -> bool {
    matches!(try_lock(paths, server_id), Ok(None))
}

/// The ids of every server a shim runs for.
pub fn running(paths: &Paths) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(paths.root().join("runs")) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|id| is_running(paths, id))
        .collect()
}

/// Start a detached shim for a server whose `launch.json` is written. The child is returned so
/// Core can notice a shim that dies during start and reap it later.
pub fn spawn(paths: &Paths, server_id: &str) -> io::Result<Child> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["shim", "--data-dir"])
        .arg(paths.root())
        .arg(server_id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(File::create(paths.shim_log(server_id))?);
    #[cfg(unix)]
    {
        // Out of Core's process group, so a Ctrl+C for Core does not reach it.
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        command.spawn()
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        use windows_sys::Win32::{
            Foundation::ERROR_ACCESS_DENIED,
            System::Threading::{
                CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP,
                DETACHED_PROCESS,
            },
        };

        // Leave any job Core runs in, so the shim does not die with it. A job that forbids
        // leaving it refuses with "access denied"; then the shim stays in it.
        let detached = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        command.creation_flags(detached | CREATE_BREAKAWAY_FROM_JOB);
        match command.spawn() {
            Err(error)
                if error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) =>
            {
                tracing::warn!(
                    "Core runs in a job that keeps its children; servers stop when it does"
                );
                command.creation_flags(detached);
                command.spawn()
            }
            result => result,
        }
    }
}

/// A connection to a shim, after its greeting.
pub struct Connection {
    stream: BufReader<Box<dyn Stream>>,
    /// The run the shim is supervising.
    pub run_id: String,
}

impl Connection {
    /// Connect to a server's shim and read its greeting.
    pub async fn open(
        paths: &Paths,
        server_id: &str,
    ) -> Result<Self, ShimError> {
        let stream = connect_to(&paths.shim_socket(server_id)).await?;
        let mut stream = BufReader::new(stream);
        let mut line = String::new();
        let read = tokio::time::timeout(
            Duration::from_secs(5),
            (&mut stream).take(64 * 1024).read_line(&mut line),
        )
        .await
        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))??;
        if read == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
        }
        match decode(&line)? {
            Ok(Message::Hello { run_id }) => Ok(Self { stream, run_id }),
            Ok(other) => {
                Err(ShimError::Invalid(format!("{other:?} before hello")))
            }
            Err(version) => Err(ShimError::Incompatible(version)),
        }
    }

    pub async fn send(&mut self, message: Message) -> Result<(), ShimError> {
        let stream = self.stream.get_mut();
        stream.write_all(encode(message).as_bytes()).await?;
        stream.flush().await?;
        Ok(())
    }

    /// The shim's next message, waiting at most `limit`.
    pub async fn recv(
        &mut self,
        limit: Duration,
    ) -> Result<Message, ShimError> {
        let mut line = String::new();
        let read = tokio::time::timeout(
            limit,
            (&mut self.stream).take(64 * 1024).read_line(&mut line),
        )
        .await
        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))??;
        if read == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
        }
        decode(&line)?.map_err(ShimError::Incompatible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> RunRecord {
        RunRecord {
            v: PROTOCOL,
            run_id: "run".into(),
            pid: 42,
            start_time: 1000,
            boot_id: "boot".into(),
            java_pid: 43,
        }
    }

    #[test]
    fn a_run_is_adopted_only_when_pid_start_time_and_boot_match() {
        let record = record();
        assert!(record.is_alive("boot", Some(1000)));
        // The pid was reused by a process that started later.
        assert!(!record.is_alive("boot", Some(2000)));
        // The machine restarted; the same pid and start time mean nothing now.
        assert!(!record.is_alive("other boot", Some(1000)));
        assert!(!record.is_alive("boot", None));
    }

    #[test]
    fn messages_of_another_version_are_told_apart() {
        let line = encode(Message::Command {
            line: "say hi".into(),
        });
        assert_eq!(
            decode(&line).unwrap().unwrap(),
            Message::Command {
                line: "say hi".into()
            }
        );
        let newer = r#"{"v":2,"type":"hello","run_id":"x","new_field":true}"#;
        assert_eq!(decode(newer).unwrap().unwrap_err(), 2);
        let unknown = r#"{"v":2,"type":"something_new"}"#;
        assert_eq!(decode(unknown).unwrap().unwrap_err(), 2);
    }

    #[test]
    fn files_of_another_version_are_incompatible() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.json");
        write_json(&path, &record()).unwrap();
        assert_eq!(read_versioned::<RunRecord>(&path).unwrap(), record());
        std::fs::write(&path, r#"{"v":2,"something":"else"}"#).unwrap();
        assert!(matches!(
            read_versioned::<RunRecord>(&path),
            Err(ShimError::Incompatible(2))
        ));
    }

    #[test]
    fn a_held_run_lock_keeps_a_second_shim_out() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let held = try_lock(&paths, "a").unwrap().expect("free at first");
        assert!(try_lock(&paths, "a").unwrap().is_none());
        assert!(is_running(&paths, "a"));
        assert!(!is_running(&paths, "b"));
        assert_eq!(running(&paths), ["a"]);
        drop(held);
        assert!(!is_running(&paths, "a"));
    }
}
