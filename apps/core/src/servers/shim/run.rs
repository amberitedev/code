//! The shim process. `copal shim --data-dir <folder> <server>` lands here; see the parent module
//! for what it does and why.

use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use color_eyre::eyre::{Result, WrapErr, bail, eyre};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use super::{
    EndedBy, ExitStatus, Launch, MAX_SAVE_HOLD_S, Message, PROTOCOL, RunRecord,
    decode, encode,
    java::{self, Java, Killer, RunAs},
    read_versioned, try_lock, write_json,
};
use crate::{
    access::local::{Listener, Stream},
    daemon::{install::InstallRecord, paths::Paths},
    servers::{
        console::{MAX_LINE, MAX_LOG, read_line},
        process,
    },
};

/// Supervise one run of a server until Java exits.
pub async fn main(data_dir: PathBuf, server_id: String) -> Result<()> {
    #[cfg(unix)]
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o027));
    let paths = Paths::new(data_dir);
    let Some(_lock) = try_lock(&paths, &server_id)? else {
        bail!("a shim already runs server {server_id}");
    };
    let launch: Launch = read_versioned(&paths.launch_spec(&server_id))
        .wrap_err("could not read launch.json")?;
    let status_path = paths.run_status(&server_id);
    let _ = std::fs::remove_file(&status_path);
    let status = |exit_code, ended_by, error| {
        write_json(
            &status_path,
            &ExitStatus {
                v: PROTOCOL,
                run_id: launch.run_id.clone(),
                exit_code,
                ended_at: chrono::Utc::now().timestamp(),
                ended_by,
                error,
            },
        )
    };

    let console =
        Arc::new(Mutex::new(Console::create(&paths.console_log(&server_id))?));
    // The socket comes first: a server nobody could stop must not start.
    let socket = paths.shim_socket(&server_id);
    let listener = match listen(&socket) {
        Ok(listener) => listener,
        Err(error) => {
            let error =
                eyre!("could not listen on {}: {error}", socket.display());
            status(None, EndedBy::Exit, Some(format!("{error:#}")))?;
            return Err(error);
        }
    };
    let java =
        run_as(&paths).and_then(|run_as| Ok(java::spawn(&launch, &run_as)?));
    let Java {
        pid: java_pid,
        stdin,
        stdout,
        stderr,
        killer,
        waiter,
    } = match java {
        Ok(java) => java,
        Err(error) => {
            status(None, EndedBy::Exit, Some(format!("{error:#}")))?;
            return Err(error);
        }
    };
    eprintln!("run {}: Java runs as pid {java_pid}", launch.run_id);
    let pumps = [
        pump(stdout, Arc::clone(&console)),
        pump(stderr, Arc::clone(&console)),
    ];

    let shared = Arc::new(Shared {
        run_id: launch.run_id.clone(),
        stdin: Mutex::new(stdin),
        killer,
        ended_by: Mutex::new(EndedBy::Exit),
        saves: Arc::default(),
    });
    let server = tokio::spawn(serve(listener, Arc::clone(&shared)));
    let pid = std::process::id();
    let record = process::start_time(pid)
        .ok_or_else(|| eyre!("cannot read the shim's own start time"))
        .and_then(|start_time| {
            let record = RunRecord {
                v: PROTOCOL,
                run_id: launch.run_id.clone(),
                pid,
                start_time,
                boot_id: process::boot_id()?,
                java_pid,
            };
            Ok(write_json(&paths.run_record(&server_id), &record)?)
        });
    if let Err(error) = record {
        // Without run.json Core cannot adopt this run, so it must not keep running unseen.
        eprintln!("could not write run.json: {error:#}");
        shared.act(Message::Kill);
    }

    let exit = tokio::task::spawn_blocking(move || waiter.wait()).await?;
    server.abort();
    // A process Java started may still hold its output open; that must not keep the shim up.
    let deadline = Instant::now() + Duration::from_secs(5);
    while pumps.iter().any(|pump| !pump.is_finished())
        && Instant::now() < deadline
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let ended_by = *shared.ended_by.lock().expect("not poisoned");
    eprintln!("run {}: Java exited: {exit:?}, {ended_by:?}", launch.run_id);
    match exit {
        Ok(code) => status(Some(code), ended_by, None)?,
        Err(error) => status(None, ended_by, Some(error.to_string()))?,
    }
    #[cfg(unix)]
    let _ = std::fs::remove_file(&socket);
    Ok(())
}

/// As `copal-servers` once Copal is installed; as the shim's own account in development.
fn run_as(paths: &Paths) -> Result<RunAs> {
    Ok(match InstallRecord::load(paths)? {
        None => RunAs::Shim,
        #[cfg(windows)]
        Some(record) => RunAs::Servers {
            password: record.servers_password.ok_or_else(|| {
                eyre!("install.toml has no password for copal-servers")
            })?,
        },
        #[cfg(unix)]
        Some(_) => RunAs::Servers {},
    })
}

/// The command socket: inside `sockets/`, which only `copal` can enter, or a pipe only `copal`
/// may open.
fn listen(path: &Path) -> std::io::Result<Listener> {
    #[cfg(unix)]
    return Listener::bind(path, 0o600);
    #[cfg(windows)]
    return Listener::bind(
        path,
        &format!("D:P(A;;GA;;;{})", crate::daemon::win::current_sid()?),
    );
}

/// What every connection shares.
struct Shared {
    run_id: String,
    stdin: Mutex<Box<dyn Write + Send>>,
    killer: Killer,
    ended_by: Mutex<EndedBy>,
    saves: Arc<SaveHold>,
}

impl Shared {
    /// Carry out one of Core's messages, and return the answer it gets, if any.
    fn act(self: &Arc<Self>, message: Message) -> Option<Message> {
        let result = match message {
            Message::Command { line } => {
                self.write(line.lines().next().unwrap_or_default())
            }
            Message::Stop => {
                let mut ended_by = self.ended_by.lock().expect("not poisoned");
                if *ended_by == EndedBy::Exit {
                    *ended_by = EndedBy::Stop;
                }
                drop(ended_by);
                self.write("stop")
            }
            Message::Kill => {
                *self.ended_by.lock().expect("not poisoned") = EndedBy::Kill;
                self.killer.kill()
            }
            Message::HoldSaves { seconds } => {
                let seconds = seconds.min(MAX_SAVE_HOLD_S);
                let shared = Arc::clone(self);
                let held = self
                    .saves
                    .hold(Duration::from_secs(seconds), move |line| {
                        shared.write(line)
                    });
                return answer(held.map(|()| Message::SavesHeld { seconds }));
            }
            Message::ReleaseSaves => {
                let released = self.saves.release(|line| self.write(line));
                return answer(released.map(|()| Message::SavesReleased));
            }
            Message::Hello { .. }
            | Message::SavesHeld { .. }
            | Message::SavesReleased => Ok(()),
        };
        if let Err(error) = result {
            eprintln!("{error}");
        }
        None
    }

    fn write(&self, line: &str) -> std::io::Result<()> {
        let mut stdin = self.stdin.lock().expect("not poisoned");
        stdin.write_all(line.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()
    }
}

/// The answer to a hold or release, or nothing when it failed (Core then gives up waiting).
fn answer(result: std::io::Result<Message>) -> Option<Message> {
    result.map_err(|error| eprintln!("{error}")).ok()
}

/// Autosave turned off for a backup. Core turns it back on; if Core never comes back, the hold
/// runs out and the shim does, so saving is never left off.
///
/// Every change of the hold is made together with its console line, under one lock: a release
/// on one connection and a hold on another, or a hold running out, can never leave the last
/// line Java got disagreeing with the state.
#[derive(Default)]
struct SaveHold {
    /// Which hold is current, and whether one is.
    state: Mutex<(u64, bool)>,
}

impl SaveHold {
    /// Send `save-off` through `write`, and `save-on` after `limit` unless released or held again
    /// before then.
    fn hold(
        self: &Arc<Self>,
        limit: Duration,
        write: impl Fn(&str) -> std::io::Result<()> + Send + 'static,
    ) -> std::io::Result<()> {
        let generation = {
            let mut state = self.state.lock().expect("not poisoned");
            write("save-off")?;
            state.0 += 1;
            state.1 = true;
            state.0
        };
        let hold = Arc::clone(self);
        std::thread::spawn(move || {
            std::thread::sleep(limit);
            if let Err(error) = hold.end(Some(generation), &write) {
                eprintln!("{error}");
            }
        });
        Ok(())
    }

    /// Send `save-on` now if saving is held off.
    fn release(
        &self,
        write: impl Fn(&str) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        self.end(None, write)
    }

    /// End the hold `generation`, or whichever is current, sending `save-on` if one ends.
    fn end(
        &self,
        generation: Option<u64>,
        write: impl Fn(&str) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let mut state = self.state.lock().expect("not poisoned");
        if state.1 && generation.is_none_or(|generation| generation == state.0)
        {
            write("save-on")?;
            state.1 = false;
        }
        Ok(())
    }
}

async fn serve(mut listener: Listener, shared: Arc<Shared>) {
    loop {
        match listener.accept().await {
            Ok(stream) => {
                tokio::spawn(connection(stream, Arc::clone(&shared)));
            }
            Err(error) => {
                eprintln!("command socket: {error}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn connection(stream: Box<dyn Stream>, shared: Arc<Shared>) {
    let (reader, mut writer) = tokio::io::split(stream);
    let hello = encode(Message::Hello {
        run_id: shared.run_id.clone(),
    });
    if writer.write_all(hello.as_bytes()).await.is_err() {
        return;
    }
    let mut reader = tokio::io::BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        match (&mut reader)
            .take(MAX_LINE as u64)
            .read_line(&mut line)
            .await
        {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        match decode(&line) {
            Ok(Ok(message)) => {
                let shared = Arc::clone(&shared);
                let reply =
                    tokio::task::spawn_blocking(move || shared.act(message))
                        .await;
                if let Ok(Some(reply)) = reply
                    && writer.write_all(encode(reply).as_bytes()).await.is_err()
                {
                    return;
                }
            }
            Ok(Err(version)) => {
                eprintln!("ignored a message of shim protocol {version}");
            }
            Err(error) => {
                eprintln!("{error}");
                return;
            }
        }
    }
}

/// `console.log`, started over when it reaches [`MAX_LOG`].
struct Console {
    file: File,
    size: u64,
}

impl Console {
    fn create(path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            file: File::create(path)?,
            size: 0,
        })
    }

    fn write(&mut self, line: &[u8]) -> std::io::Result<()> {
        if self.size >= MAX_LOG {
            self.file.set_len(0)?;
            self.file.seek(SeekFrom::Start(0))?;
            self.size = 0;
        }
        self.file.write_all(line)?;
        self.size += line.len() as u64;
        Ok(())
    }
}

/// Copy one of Java's output streams to the console, line by line, on a thread of its own.
fn pump(
    stream: Box<dyn Read + Send>,
    console: Arc<Mutex<Console>>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut line = Vec::new();
        loop {
            line.clear();
            match read_line(&mut reader, &mut line) {
                Ok((0, _)) | Err(_) => return,
                Ok(_) => {}
            }
            if !line.ends_with(b"\n") {
                line.push(b'\n');
            }
            if let Err(error) =
                console.lock().expect("not poisoned").write(&line)
            {
                eprintln!("console.log: {error}");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorder() -> (
        Arc<Mutex<Vec<String>>>,
        impl Fn(&str) -> std::io::Result<()> + Send + Sync + Clone + 'static,
    ) {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&lines);
        (lines, move |line: &str| {
            sink.lock().unwrap().push(line.to_owned());
            Ok(())
        })
    }

    #[test]
    fn holds_and_releases_from_many_connections_end_with_saving_on() {
        let saves = Arc::new(SaveHold::default());
        let (lines, write) = recorder();
        let threads: Vec<_> = (0..16)
            .map(|_| {
                let saves = Arc::clone(&saves);
                let write = write.clone();
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        saves
                            .hold(Duration::from_secs(60), write.clone())
                            .unwrap();
                        saves.release(write.clone()).unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let lines = lines.lock().unwrap();
        // Java got the lines in the order the hold changed: never two `save-on` in a row, and
        // the last says saving is on, as the state does.
        assert!(lines.windows(2).all(|pair| pair != ["save-on", "save-on"]));
        assert_eq!(lines.last().unwrap(), "save-on");
        assert!(!saves.state.lock().unwrap().1);
    }

    #[test]
    fn a_hold_turns_saving_off_until_released() {
        let saves = Arc::new(SaveHold::default());
        let (lines, write) = recorder();
        saves.hold(Duration::from_secs(60), write.clone()).unwrap();
        saves.release(write.clone()).unwrap();
        // A second release has nothing to end.
        saves.release(write).unwrap();
        assert_eq!(*lines.lock().unwrap(), ["save-off", "save-on"]);
    }

    #[test]
    fn a_hold_core_never_releases_runs_out() {
        let saves = Arc::new(SaveHold::default());
        let (lines, write) = recorder();
        saves
            .hold(Duration::from_millis(50), write.clone())
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(*lines.lock().unwrap(), ["save-off", "save-on"]);
        // Releasing afterwards sends nothing more.
        saves.release(write).unwrap();
        assert_eq!(lines.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_second_hold_starts_the_time_again() {
        let saves = Arc::new(SaveHold::default());
        let (lines, write) = recorder();
        saves
            .hold(Duration::from_millis(50), write.clone())
            .unwrap();
        saves.hold(Duration::from_secs(60), write.clone()).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            *lines.lock().unwrap(),
            ["save-off", "save-off"],
            "the first limit is gone"
        );
        saves.release(write).unwrap();
        assert_eq!(lines.lock().unwrap().last().unwrap(), "save-on");
    }
}
