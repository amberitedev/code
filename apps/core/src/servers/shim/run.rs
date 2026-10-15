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
    EndedBy, ExitStatus, Launch, Message, PROTOCOL, RunRecord, decode, encode,
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
    });
    let server = tokio::spawn(serve(listener, Arc::clone(&shared)));
    let pid = std::process::id();
    write_json(
        &paths.run_record(&server_id),
        &RunRecord {
            v: PROTOCOL,
            run_id: launch.run_id.clone(),
            pid,
            start_time: process::start_time(pid).ok_or_else(|| {
                eyre!("cannot read the shim's own start time")
            })?,
            boot_id: process::boot_id()?,
            java_pid,
        },
    )?;

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
}

impl Shared {
    fn act(&self, message: Message) {
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
            Message::Hello { .. } => Ok(()),
        };
        if let Err(error) = result {
            eprintln!("{error}");
        }
    }

    fn write(&self, line: &str) -> std::io::Result<()> {
        let mut stdin = self.stdin.lock().expect("not poisoned");
        stdin.write_all(line.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()
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
                let _ =
                    tokio::task::spawn_blocking(move || shared.act(message))
                        .await;
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
