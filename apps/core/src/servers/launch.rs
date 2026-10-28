//! Preparing a server to start: its folder, `eula.txt`, its port in `server.properties`, Java,
//! and the command line the shim runs, for every loader.

use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use color_eyre::eyre::Result;

use super::{
    records::ServerRow,
    shim::{Launch, PROTOCOL},
};
use crate::{
    daemon::{folders, paths::Paths},
    installs::{
        ModLoader, forge, java,
        shared::{
            SharedDirs, launch::build_launch_tokens,
            version_info::merged_version_id,
        },
    },
};

/// Why a server cannot start yet.
#[derive(Debug, thiserror::Error)]
pub enum NotReady {
    #[error("nothing is installed on this server yet")]
    NotInstalled,
    #[error(
        "accept the Minecraft EULA for this server first: copal servers eula <id>"
    )]
    Eula,
}

/// What is installed on a server, or why nothing is.
fn installed(server: &ServerRow) -> Result<(ModLoader, &str), NotReady> {
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    match (loader, server.game_version.as_deref()) {
        (Some(loader), Some(game_version)) => Ok((loader, game_version)),
        _ => Err(NotReady::NotInstalled),
    }
}

/// Everything the shim needs to start `server`, and the lease on its Java (see `java::Lease`).
/// Writes `eula.txt` and the port first.
pub async fn prepare(
    paths: &Paths,
    http: &reqwest::Client,
    server: &ServerRow,
    run_id: &str,
) -> Result<(Launch, Option<java::Lease>)> {
    installed(server)?;
    if !server.eula_accepted {
        return Err(NotReady::Eula.into());
    }

    let dir = paths.server(&server.id);
    folders::create_server_folder(&dir)?;
    folders::create_server_folder(&dir.join("tmp"))?;
    write_server_file(
        &dir.join("eula.txt"),
        b"eula=true
",
    )?;
    set_port(&dir, server.port)?;

    let (java, lease) = java_for(paths, http, server).await?;
    let memory = server.memory_mb;
    let launch = Launch {
        v: PROTOCOL,
        run_id: run_id.to_owned(),
        java,
        args: args(paths, server)?,
        dir,
        memory_limit_mb: memory_limit(memory),
    };
    Ok((launch, lease))
}

/// The arguments after `java`: memory, the temporary folder, and what the loader starts from.
pub fn args(paths: &Paths, server: &ServerRow) -> Result<Vec<String>> {
    let (loader, game_version) = installed(server)?;
    let dir = paths.server(&server.id);
    let memory = server.memory_mb;
    // The heap and tuned flags first: `memory::use_flags` replaces those in place.
    let mut args = super::memory::jvm(memory);
    args.extend([
        format!("-Djava.io.tmpdir={}", dir.join("tmp").display()),
        // `copal-servers` has no home folder.
        format!("-Duser.home={}", dir.display()),
    ]);
    let dirs = SharedDirs::new(paths.root());
    let loader_version = server.loader_version.as_deref().unwrap_or_default();
    match loader {
        ModLoader::Vanilla | ModLoader::Fabric | ModLoader::Quilt => {
            let version_id =
                merged_version_id(game_version, loader, Some(loader_version));
            let info = std::fs::read(dirs.version_info_path(&version_id))
                .map_err(|_| NotReady::NotInstalled)?;
            let tokens = build_launch_tokens(
                &dirs,
                &serde_json::from_slice(&info)?,
                game_version,
                loader,
                std::env::consts::ARCH,
            )?
            .ok_or(NotReady::NotInstalled)?;
            args.extend(tokens);
        }
        ModLoader::Paper => {
            let jar = dirs.paper_jar(game_version, loader_version);
            if !jar.is_file() {
                return Err(NotReady::NotInstalled.into());
            }
            args.extend([
                "-jar".into(),
                jar.to_string_lossy().into_owned(),
                "--nogui".into(),
            ]);
        }
        ModLoader::Forge | ModLoader::NeoForge => {
            args.extend(forge::launch_args(
                &dirs,
                &dir,
                loader,
                game_version,
                loader_version,
            )?)
        }
    }
    Ok(args)
}

/// The Java major `server` runs on: the one the owner chose, or the one its Minecraft version
/// needs.
pub async fn java_major(
    paths: &Paths,
    http: &reqwest::Client,
    server: &ServerRow,
) -> Result<u32> {
    if let Some(major) = server.java_version {
        return Ok(u32::try_from(major)?);
    }
    let (_, game_version) = installed(server)?;
    Ok(java::java_requirement(http, paths.root(), game_version).await)
}

/// The Java `server` runs with: its custom path, or the managed Java of its major, downloaded
/// when missing.
pub async fn java_for(
    paths: &Paths,
    http: &reqwest::Client,
    server: &ServerRow,
) -> Result<(PathBuf, Option<java::Lease>)> {
    if let Some(path) = &server.java_path {
        return Ok((PathBuf::from(path), None));
    }
    let major = java_major(paths, http, server).await?;
    let (binary, lease) = java::ensure(paths, http, major).await?;
    Ok((binary, Some(lease)))
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

/// Set `server-port` in `server.properties`, keeping every other line.
fn set_port(dir: &Path, port: i64) -> io::Result<()> {
    let path = dir.join("server.properties");
    let text = match read_server_file(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let mut lines: Vec<&str> = text
        .lines()
        .filter(|line| property_key(line) != Some("server-port"))
        .collect();
    let port_line = format!("server-port={port}");
    lines.push(&port_line);
    write_server_file(&path, format!("{}\n", lines.join("\n")).as_bytes())
}

/// The key of a `server.properties` line, or `None` for comments and blank lines.
pub fn property_key(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if line.is_empty() || line.starts_with(['#', '!']) {
        return None;
    }
    let end = line.find(['=', ':']).unwrap_or(line.len());
    Some(line[..end].trim_end())
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
    use super::*;

    #[test]
    fn the_port_replaces_only_its_own_line() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("server.properties"),
            "#Minecraft server properties\nmotd=Hi\nserver-port = 25565\nserver-portal=x\n",
        )
        .unwrap();
        set_port(dir.path(), 25570).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("server.properties"))
                .unwrap(),
            "#Minecraft server properties\nmotd=Hi\nserver-portal=x\nserver-port=25570\n"
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
        assert!(set_port(dir.path(), 25565).is_err());
    }

    #[test]
    fn the_memory_cap_leaves_room_outside_the_heap() {
        assert_eq!(memory_limit(2048), 3072);
        assert_eq!(memory_limit(8192), 12288);
    }
}
