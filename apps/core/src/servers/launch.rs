//! Preparing a server to start: its folder, `eula.txt`, its address in `server.properties`, Java,
//! and the command line the shim runs: the engine's command for the server, after Core's own
//! JVM flags.

use std::{
    io::{self, Read, Write},
    net::IpAddr,
    path::{Path, PathBuf},
};

use color_eyre::eyre::{Result, eyre};
pub use engine::sandbox::property_key;
use engine::{Engine, server::ServerCommand};

use super::shim::{Launch, PROTOCOL};
use super::{platform::Platform, server_rows::ServerRow};
use crate::daemon::{folders, paths::Paths};

/// Why a server cannot start yet.
#[derive(Debug, thiserror::Error)]
pub enum NotReady {
    #[error("nothing is installed on this server yet")]
    NotInstalled,
}

/// Everything the shim needs to start `server`, with `flags` (`memory::flags`) as its JVM flags.
/// Writes `eula.txt` and the address first. `local_only` is `Config::local_only`.
pub async fn prepare(
    paths: &Paths,
    engine: &Engine,
    server: &ServerRow,
    platform: Option<&Platform>,
    flags: Option<Vec<String>>,
    run_id: &str,
    local_only: Option<IpAddr>,
) -> Result<Launch> {
    platform.ok_or(NotReady::NotInstalled)?;

    let dir = paths.server(&server.id);
    folders::create_server_folder(&dir)?;
    folders::create_server_folder(&dir.join("tmp"))?;
    write_server_file(
        &dir.join("eula.txt"),
        b"eula=true
",
    )?;
    set_address(&dir, server.port, local_only)?;

    let command = engine::server::server_command(engine, &server.id)
        .await
        .map_err(|error| eyre!("{error:#}"))?;
    let java = java_for(engine, server, &command).await?;
    Ok(launch(paths, server, java, command, flags, run_id))
}

/// What the shim runs: `java`, Core's flags, then the engine's arguments, in the engine's folder.
fn launch(
    paths: &Paths,
    server: &ServerRow,
    java: PathBuf,
    command: ServerCommand,
    flags: Option<Vec<String>>,
    run_id: &str,
) -> Launch {
    Launch {
        v: PROTOCOL,
        run_id: run_id.to_owned(),
        java,
        args: args(paths, server, flags, command.args),
        dir: command.cwd,
        memory_limit_mb: memory_limit(server.memory_mb),
    }
}

/// The arguments after `java`: the heap, the JVM flags (`flags`, or the tuned ones), the
/// temporary folder and home, then `server_args`, which the loader starts from.
pub fn args(
    paths: &Paths,
    server: &ServerRow,
    flags: Option<Vec<String>>,
    server_args: Vec<String>,
) -> Vec<String> {
    let dir = paths.server(&server.id);
    let mut args = super::memory::jvm(server.memory_mb);
    if let Some(flags) = flags {
        super::memory::use_flags(&mut args, flags);
    }
    args.extend([
        format!("-Djava.io.tmpdir={}", dir.join("tmp").display()),
        // `copal-servers` has no home folder.
        format!("-Duser.home={}", dir.display()),
    ]);
    args.extend(server_args);
    args
}

/// The Java `server` runs: its custom path, the engine's Java of the major its owner chose, or
/// the one its Minecraft version needs, which the engine chose for `command`.
async fn java_for(
    engine: &Engine,
    server: &ServerRow,
    command: &ServerCommand,
) -> Result<PathBuf> {
    if let Some(path) = &server.java_path {
        return Ok(PathBuf::from(path));
    }
    let Some(major) = server.java_version else {
        return Ok(command.java.clone());
    };
    let major = u32::try_from(major)?;
    let installed = engine::jre::get_java_versions(engine)
        .await
        .map_err(|error| eyre!("{error:#}"))?
        .get(&major)
        .map(|java| PathBuf::from(&java.path));
    if let Some(path) = installed {
        return Ok(path);
    }
    let path =
        engine::jre::auto_install_java_with_loading(engine, major, false)
            .await
            .map_err(|error| eyre!("{error:#}"))?;
    let java = engine::jre::check_jre(path.clone())
        .await
        .map_err(|error| eyre!("{error:#}"))?;
    engine::jre::set_java_version(engine, java)
        .await
        .map_err(|error| eyre!("{error:#}"))?;
    Ok(path)
}

/// The Java major `server` runs `game_version` on: the one the owner chose, or the one the
/// Minecraft version needs.
pub async fn java_major(
    engine: &Engine,
    server: &ServerRow,
    game_version: &str,
) -> Result<u32> {
    if let Some(major) = server.java_version {
        return Ok(u32::try_from(major)?);
    }
    engine::server::java_major(engine, game_version)
        .await
        .map_err(|error| eyre!("{error:#}"))
}

/// The command line as people read it: `java` and the arguments, quoted where needed.
pub fn invocation(args: &[String]) -> String {
    std::iter::once("java")
        .chain(args.iter().map(String::as_str))
        .map(|arg| {
            if arg.contains(char::is_whitespace) {
                format!("\"{arg}\"")
            } else {
                arg.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The job object's cap on Windows: the heap, plus room for what Java keeps outside it.
pub fn memory_limit(heap_mb: i64) -> u64 {
    let heap = heap_mb.max(0) as u64;
    heap + (heap / 2).max(1024)
}

/// Set `server-port` in `server.properties`, and `server-ip` for a local-only Core so Minecraft
/// listens on loopback too. Every other line stays, otherwise the owner's `server-ip` included.
fn set_address(
    dir: &Path,
    port: i64,
    local_only: Option<IpAddr>,
) -> io::Result<()> {
    let path = dir.join("server.properties");
    let text = match read_server_file(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let ours = |key: &str| {
        key == "server-port" || (key == "server-ip" && local_only.is_some())
    };
    let mut lines: Vec<String> = text
        .lines()
        .filter(|line| !property_key(line).is_some_and(ours))
        .map(str::to_owned)
        .collect();
    lines.push(format!("server-port={port}"));
    if let Some(ip) = local_only {
        lines.push(format!("server-ip={ip}"));
    }
    write_server_file(&path, format!("{}\n", lines.join("\n")).as_bytes())
}

/// Files in a server folder are written by mods too: never follow a link they planted, and
/// read at most 1 MB. Only for files directly in a folder Core made, such as the server folder
/// itself: a link in a folder above the file is not checked.
pub fn read_server_file(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    no_follow(std::fs::File::options().read(true))
        .open(path)
        .and_then(regular)?
        .take(1024 * 1024)
        .read_to_string(&mut text)?;
    Ok(text)
}

/// Create or replace a file in a server folder. It is truncated only once the open handle is
/// known to be a plain file, never through a link. Same limits as [`read_server_file`].
pub fn write_server_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = std::fs::File::options();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o660);
    let mut file = no_follow(&mut options).open(path).and_then(regular)?;
    file.set_len(0)?;
    file.write_all(contents)
}

fn no_follow(options: &mut std::fs::OpenOptions) -> &mut std::fs::OpenOptions {
    // O_NONBLOCK: opening a FIFO a mod planted returns at once instead of waiting forever,
    // and `regular` then refuses it. It changes nothing for a plain file.
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(
        options,
        nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK,
    );
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::custom_flags(
        options,
        windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
    );
    options
}

/// Refuse anything but a plain file, such as a link opened as itself.
fn regular(file: std::fs::File) -> io::Result<std::fs::File> {
    let meta = file.metadata()?;
    #[cfg(windows)]
    let link = std::os::windows::fs::MetadataExt::file_attributes(&meta)
        & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0;
    #[cfg(unix)]
    let link = false;
    if meta.is_file() && !link {
        Ok(file)
    } else {
        Err(io::Error::other("not a plain file"))
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn the_port_replaces_only_its_own_line() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("server.properties"),
            "#Minecraft server properties\nmotd=Hi\nserver-port = 25565\nserver-portal=x\n",
        )
        .unwrap();
        set_address(dir.path(), 25570, None).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("server.properties"))
                .unwrap(),
            "#Minecraft server properties\nmotd=Hi\nserver-portal=x\nserver-port=25570\n"
        );
    }

    #[test]
    fn a_local_only_core_owns_server_ip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.properties");
        std::fs::write(&path, "server-ip=10.0.0.5\nmotd=Hi\n").unwrap();
        set_address(dir.path(), 25570, None).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "server-ip=10.0.0.5\nmotd=Hi\nserver-port=25570\n"
        );
        set_address(dir.path(), 25570, Some(Ipv4Addr::LOCALHOST.into()))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "motd=Hi\nserver-port=25570\nserver-ip=127.0.0.1\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_planted_link_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("db");
        std::fs::write(&target, "secret").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("eula.txt"))
            .unwrap();
        assert!(
            write_server_file(&dir.path().join("eula.txt"), b"eula=true\n")
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "secret");
    }

    #[cfg(unix)]
    #[test]
    fn a_planted_fifo_is_refused_without_waiting() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["eula.txt", "server.properties"] {
            nix::unistd::mkfifo(
                &dir.path().join(name),
                nix::sys::stat::Mode::from_bits_truncate(0o600),
            )
            .unwrap();
        }
        assert!(write_server_file(&dir.path().join("eula.txt"), b"x").is_err());
        assert!(set_address(dir.path(), 25565, None).is_err());
    }

    #[test]
    fn the_shim_runs_the_engines_command_after_cores_flags() {
        let paths = Paths::new("data");
        let server = ServerRow {
            id: "a".into(),
            name: "A".into(),
            port: 25565,
            memory_mb: 2048,
            java_path: None,
            java_version: None,
        };
        let dir = paths.server("a");
        let core_flags = |flags: &[&str]| {
            let mut args = vec!["-Xms512M".to_owned(), "-Xmx2048M".to_owned()];
            args.extend(flags.iter().map(|flag| (*flag).to_owned()));
            args.push(format!(
                "-Djava.io.tmpdir={}",
                dir.join("tmp").display()
            ));
            args.push(format!("-Duser.home={}", dir.display()));
            args
        };
        let vanilla = ["-jar", "/meta/versions/1.21.1/server.jar", "--nogui"];
        let fabric = [
            "-cp",
            "/meta/versions/1.21.1/server.jar:/meta/libraries/a.jar",
            "net.fabricmc.loader.impl.launch.knot.KnotServer",
            "--nogui",
        ];
        for (server_args, flags) in [
            (&vanilla[..], None),
            (&fabric[..], Some(vec!["-XX:+UseZGC".to_owned()])),
        ] {
            let command = ServerCommand {
                java: "/meta/java_versions/zulu21/bin/java".into(),
                args: server_args.iter().map(|arg| (*arg).to_owned()).collect(),
                cwd: "/data/servers/a".into(),
            };
            let launch = launch(
                &paths,
                &server,
                command.java.clone(),
                command.clone(),
                flags.clone(),
                "run",
            );
            let mut expected = match flags {
                Some(_) => core_flags(&["-XX:+UseZGC"]),
                None => core_flags(&super::super::memory::TUNED_FLAGS),
            };
            expected.extend(command.args);
            assert_eq!(launch.java, command.java);
            assert_eq!(launch.args, expected);
            assert_eq!(launch.dir, command.cwd);
        }
    }

    #[test]
    fn the_memory_cap_leaves_room_outside_the_heap() {
        assert_eq!(memory_limit(2048), 3072);
        assert_eq!(memory_limit(8192), 12288);
    }
}
