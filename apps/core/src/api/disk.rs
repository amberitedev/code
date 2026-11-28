//! What takes up space on the Core's disk, measured when asked: each server's folder and
//! backups, the store's shared files, and the game files and Java the servers share.

use std::{io, path::Path, sync::Arc};

use axum::extract::State;
use contracts::{
    ApiError, Json,
    core::{DiskUsage, GetDiskUsage, ServerUsage},
    server::Req,
};
use walkdir::WalkDir;

use super::{CoreState, internal};
use crate::{
    access::{Caller, require_owner},
    daemon::paths::Paths,
    servers::server_rows,
};

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<GetDiskUsage>,
) -> Result<Json<DiskUsage>, ApiError> {
    require_owner(&caller)?;
    let servers = server_rows::list(&state.db, &state.engine)
        .await
        .map_err(internal)?
        .into_iter()
        .map(|server| (server.id, server.name))
        .collect();
    let paths = state.servers.paths().clone();
    tokio::task::spawn_blocking(move || usage(&paths, servers))
        .await
        .map_err(internal)?
        .map(Json)
        .map_err(internal)
}

/// The size of the disk that holds `path`, and its bytes this account may still write.
pub fn space(path: &Path) -> io::Result<(u64, u64)> {
    let stats = fs4::statvfs(path)?;
    Ok((stats.total_space(), stats.available_space()))
}

/// Measure the data folder. `servers` are the ids and names of every server.
fn usage(
    paths: &Paths,
    servers: Vec<(String, String)>,
) -> io::Result<DiskUsage> {
    let (total, free) = space(paths.root())?;
    Ok(DiskUsage {
        total,
        free,
        servers: servers
            .into_iter()
            .map(|(id, name)| {
                let backups = paths.backups().join(&id);
                ServerUsage {
                    files: size(&paths.server(&id)),
                    backups: size(&backups),
                    backup_count: backup_count(&backups),
                    id,
                    name,
                }
            })
            .collect(),
        shared: size(&paths.store()),
        game_files: size(&paths.meta()),
    })
}

/// The bytes of every file under `path`, without following links. Missing is empty.
fn size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(std::fs::Metadata::is_file)
        .map(|metadata| metadata.len())
        .sum()
}

/// Finished backups: the zips in a server's backup folder (see `backups`).
fn backup_count(folder: &Path) -> u32 {
    let count = std::fs::read_dir(folder).map_or(0, |entries| {
        entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().is_some_and(|ext| ext == "zip")
            })
            .count()
    });
    u32::try_from(count).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn each_folder_is_measured_on_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let server = paths.server("a");
        fs::create_dir_all(server.join("world")).unwrap();
        fs::write(server.join("world/level.dat"), [0; 100]).unwrap();
        fs::write(server.join("server.properties"), [0; 20]).unwrap();
        let backups = paths.backups().join("a");
        fs::create_dir_all(&backups).unwrap();
        fs::write(backups.join("one.zip"), [0; 30]).unwrap();
        fs::write(backups.join("two.zip.part"), [0; 5]).unwrap();
        fs::create_dir_all(paths.store()).unwrap();
        fs::write(paths.store().join("blob"), [0; 7]).unwrap();

        let usage = usage(&paths, vec![("a".into(), "SMP".into())]).unwrap();
        assert_eq!(
            usage.servers,
            vec![ServerUsage {
                id: "a".into(),
                name: "SMP".into(),
                files: 120,
                backups: 35,
                backup_count: 1,
            }]
        );
        assert_eq!((usage.shared, usage.game_files), (7, 0));
        assert!(usage.total >= usage.free && usage.free > 0);
    }
}
