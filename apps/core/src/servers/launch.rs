//! Preparing a server to start: its folder, `eula.txt`, its port in `server.properties`, Java,
//! and the command line the shim runs. Only Vanilla so far; the other loaders are step 4.

use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use color_eyre::eyre::{Result, eyre};

use super::{
    records::ServerRow,
    shim::{Launch, PROTOCOL},
};
use crate::{
    daemon::{folders, paths::Paths},
    installs::{
        ModLoader,
        daedalus::{self, SharedDirs, launch::build_launch_tokens},
        java,
    },
};

/// Why a server cannot start yet.
#[derive(Debug, thiserror::Error)]
pub enum NotReady {
    #[error("nothing is installed on this server yet")]
    NotInstalled,
    #[error("only Vanilla servers can start so far")]
    Unsupported,
    #[error(
        "accept the Minecraft EULA for this server first: copal servers eula <id>"
    )]
    Eula,
}

/// The heap a server starts with. It grows to `memory_mb` as needed.
const START_HEAP_MB: i64 = 512;

/// Install Vanilla `game_version` into the shared store, with the Java it needs.
pub async fn install_vanilla(
    paths: &Paths,
    http: &reqwest::Client,
    game_version: &str,
) -> Result<()> {
    daedalus::install_shared(
        http,
        paths.root(),
        &ModLoader::Vanilla,
        game_version,
        None,
        None,
        std::env::consts::ARCH,
    )
    .await?;
    managed_java(paths, http, game_version).await?;
    Ok(())
}

/// Everything the shim needs to start `server`. Writes `eula.txt` and the port first.
pub async fn prepare(
    paths: &Paths,
    http: &reqwest::Client,
    server: &ServerRow,
    run_id: &str,
) -> Result<Launch> {
    let game_version = server
        .game_version
        .as_deref()
        .ok_or(NotReady::NotInstalled)?;
    if server.loader.as_deref() != Some("vanilla") {
        return Err(NotReady::Unsupported.into());
    }
    if !server.eula_accepted {
        return Err(NotReady::Eula.into());
    }

    let dir = paths.server(&server.id);
    folders::create_server_folder(&dir)?;
    folders::create_server_folder(&dir.join("tmp"))?;
    write_server_file(&dir.join("eula.txt"), b"eula=true\n")?;
    set_port(&dir, server.port)?;

    let java = match &server.java_path {
        Some(path) => PathBuf::from(path),
        None => managed_java(paths, http, game_version).await?,
    };
    let dirs = SharedDirs::new(paths.root());
    let info = tokio::fs::read(dirs.version_info_path(game_version)).await?;
    let tokens = build_launch_tokens(
        &dirs,
        &serde_json::from_slice(&info)?,
        game_version,
        &ModLoader::Vanilla,
        std::env::consts::ARCH,
    )?
    .ok_or(NotReady::Unsupported)?;

    let memory = server.memory_mb;
    let mut args = vec![
        format!("-Xms{}M", START_HEAP_MB.min(memory)),
        format!("-Xmx{memory}M"),
        format!("-Djava.io.tmpdir={}", dir.join("tmp").display()),
        // `copal-servers` has no home folder.
        format!("-Duser.home={}", dir.display()),
    ];
    args.extend(tokens);
    Ok(Launch {
        v: PROTOCOL,
        run_id: run_id.to_owned(),
        java,
        args,
        dir,
        memory_limit_mb: memory_limit(memory),
    })
}

/// The job object's cap on Windows: the heap, plus room for what Java keeps outside it.
fn memory_limit(heap_mb: i64) -> u64 {
    let heap = heap_mb.max(0) as u64;
    heap + (heap / 2).max(1024)
}

/// The Java a Minecraft version needs, downloaded into `java/` when missing.
async fn managed_java(
    paths: &Paths,
    http: &reqwest::Client,
    game_version: &str,
) -> Result<PathBuf> {
    let (major, _) =
        java::java_requirement(http, paths.root(), game_version, None).await;
    let binary = if cfg!(windows) { "java.exe" } else { "java" };
    let installed = walkdir::WalkDir::new(paths.java().join(major.to_string()))
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| {
            entry.file_name() == binary
                && entry
                    .path()
                    .parent()
                    .is_some_and(|dir| dir.ends_with("bin"))
        });
    match installed {
        Some(entry) => Ok(entry.into_path()),
        None => java::download_java(http, paths.root(), major)
            .await
            .map_err(|error| eyre!("could not install Java {major}: {error}")),
    }
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
fn property_key(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if line.is_empty() || line.starts_with(['#', '!']) {
        return None;
    }
    let end = line.find(['=', ':']).unwrap_or(line.len());
    Some(line[..end].trim_end())
}

/// Files in a server folder are written by mods too: never follow a link they planted, and
/// read at most 1 MB.
fn read_server_file(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    no_follow(std::fs::File::options().read(true))
        .open(path)
        .and_then(regular)?
        .take(1024 * 1024)
        .read_to_string(&mut text)?;
    Ok(text)
}

fn write_server_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = std::fs::File::options();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o660);
    let mut file = no_follow(&mut options).open(path).and_then(regular)?;
    file.write_all(contents)
}

fn no_follow(options: &mut std::fs::OpenOptions) -> &mut std::fs::OpenOptions {
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(
        options,
        nix::libc::O_NOFOLLOW,
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
    if file.metadata()?.is_file() {
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

    #[test]
    fn the_memory_cap_leaves_room_outside_the_heap() {
        assert_eq!(memory_limit(2048), 3072);
        assert_eq!(memory_limit(8192), 12288);
    }
}
