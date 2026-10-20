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

/// Copy a server's worlds into `backups/<server>/pre-update-<n>/` before an update, keeping
/// the last two. A plain copy; the backup engine replaces it.
pub async fn pre_update(paths: &Paths, server_id: &str) -> io::Result<PathBuf> {
    let source = paths.server(server_id);
    let folder = paths.backups().join(server_id);
    tokio::task::spawn_blocking(move || {
        let server = Folder::open(&source)?;
        let level = server.level_name();
        // Copies are numbered, each one past the highest there, so their order never depends
        // on the clock.
        std::fs::create_dir_all(&folder)?;
        let mut number = numbered(&folder)?
            .last()
            .map_or(1, |(number, _)| number + 1);
        let destination = loop {
            let candidate = folder.join(format!("pre-update-{number}"));
            match std::fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    number += 1;
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
        let copies = numbered(&folder)?;
        let excess = copies.len().saturating_sub(KEEP_PRE_UPDATE);
        for (_, old) in &copies[..excess] {
            std::fs::remove_dir_all(old)?;
        }
        Ok(destination)
    })
    .await
    .map_err(io::Error::other)?
}

/// The pre-update copies in `folder`, oldest first, by their number.
fn numbered(folder: &Path) -> io::Result<Vec<(u64, PathBuf)>> {
    let mut copies: Vec<(u64, PathBuf)> = std::fs::read_dir(folder)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let number = name.strip_prefix("pre-update-")?.parse().ok()?;
            Some((number, entry.path()))
        })
        .collect();
    copies.sort_by_key(|(number, _)| *number);
    Ok(copies)
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
        let names: Vec<_> = made
            .iter()
            .map(|path| path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["pre-update-1", "pre-update-2", "pre-update-3"]);
    }

    #[tokio::test]
    async fn copies_are_ordered_by_number_not_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::create_dir_all(paths.server("a")).unwrap();
        let folder = paths.backups().join("a");
        for number in [9, 10] {
            std::fs::create_dir_all(
                folder.join(format!("pre-update-{number}")),
            )
            .unwrap();
        }
        let made = pre_update(&paths, "a").await.unwrap();
        assert!(made.ends_with("pre-update-11"));
        assert!(!folder.join("pre-update-9").exists());
        assert!(folder.join("pre-update-10").exists());
    }
}
