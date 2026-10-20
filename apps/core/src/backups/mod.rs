//! The one queued backup engine. Step 5 builds it on the `backups` table and the `backups/`
//! folder. Until then, [`pre_update`] makes the copy a server or pack update needs first.

use std::{
    io,
    path::{Path, PathBuf},
};

use cap_std::fs::Dir;

use crate::{content::folder::Folder, daemon::paths::Paths};

/// Pre-update copies kept per server.
const KEEP_PRE_UPDATE: usize = 2;

/// Copy a server's worlds into `backups/<server>/pre-update-<time>/` before an update, keeping
/// the last two. A plain copy; the backup engine replaces it.
pub async fn pre_update(paths: &Paths, server_id: &str) -> io::Result<PathBuf> {
    let source = paths.server(server_id);
    let folder = paths.backups().join(server_id);
    tokio::task::spawn_blocking(move || {
        let server = Folder::open(&source)?;
        let level = server.level_name();
        // Names sort by time; two copies in the same instant get the next free name.
        std::fs::create_dir_all(&folder)?;
        let mut stamp =
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let destination = loop {
            let candidate = folder.join(format!("pre-update-{stamp:020}"));
            match std::fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    stamp += 1;
                }
                Err(error) => return Err(error),
            }
        };
        for world in [
            level.clone(),
            format!("{level}_nether"),
            format!("{level}_the_end"),
        ] {
            if let Ok(dir) = server.dir().open_dir(&world) {
                copy_tree(&dir, &destination.join(&world))?;
            }
        }
        let mut copies: Vec<PathBuf> = std::fs::read_dir(&folder)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("pre-update-"))
            })
            .collect();
        copies.sort();
        let excess = copies.len().saturating_sub(KEEP_PRE_UPDATE);
        for old in &copies[..excess] {
            std::fs::remove_dir_all(old)?;
        }
        Ok(destination)
    })
    .await
    .map_err(io::Error::other)?
}

/// Copy everything in `dir` to `to`. Links are skipped: a mod may have planted them.
fn copy_tree(dir: &Dir, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in dir.entries()? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        if kind.is_dir() {
            copy_tree(&entry.open_dir()?, &to.join(&name))?;
        } else if kind.is_file() {
            let mut from = entry.open()?;
            let mut file = std::fs::File::create(to.join(&name))?;
            io::copy(&mut from, &mut file)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_update_copies_the_world_and_keeps_the_last_two() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let world = paths.server("a").join("world").join("region");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("r.0.0.mca"), "chunks").unwrap();
        std::fs::write(paths.server("a").join("mods.jar"), "not the world")
            .unwrap();
        let mut made = Vec::new();
        for _ in 0..3 {
            made.push(pre_update(&paths, "a").await.unwrap());
        }
        assert_eq!(
            std::fs::read_to_string(made[2].join("world/region/r.0.0.mca"))
                .unwrap(),
            "chunks"
        );
        assert!(!made[2].join("mods.jar").exists());
        assert!(!made[0].exists());
        assert!(made[1].exists());
    }
}
