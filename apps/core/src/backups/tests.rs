//! The engine against a real in-memory database and a server folder on disk.

use std::io::Read;

use super::*;
use crate::{daemon::folders, db, servers::records as servers_records};

struct Setup {
    _dir: tempfile::TempDir,
    paths: Paths,
    db: SqlitePool,
    backups: Arc<Backups>,
    server: String,
}

async fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    folders::create(&paths, None).unwrap();
    let db = db::memory().await;
    let servers = Servers::open(paths.clone(), db.clone()).await.unwrap();
    let server = servers.create("A").await.unwrap().id;
    servers_records::set_installed(&db, &server, "vanilla", "1.21.1", None)
        .await
        .unwrap();
    let world = paths.server(&server).join("world");
    std::fs::create_dir_all(world.join("region")).unwrap();
    std::fs::write(world.join("level.dat"), "level").unwrap();
    std::fs::write(world.join("region").join("r.0.0.mca"), "chunks").unwrap();
    std::fs::write(
        paths.server(&server).join("server.properties"),
        "motd=Hi\n",
    )
    .unwrap();
    std::fs::create_dir_all(paths.server(&server).join("mods")).unwrap();
    std::fs::write(paths.server(&server).join("mods").join("a.jar"), "mod")
        .unwrap();
    let backups = Backups::open(paths.clone(), db.clone(), servers)
        .await
        .unwrap();
    Setup {
        _dir: dir,
        paths,
        db,
        backups,
        server,
    }
}

fn entries(zip: &Path) -> Vec<String> {
    let mut archive =
        zip::ZipArchive::new(std::fs::File::open(zip).unwrap()).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|index| archive.by_index(index).unwrap().name().to_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn a_backup_holds_the_world_and_settings_and_restores_them() {
    let setup = setup().await;
    let backups = &setup.backups;
    let (id, operation) = backups
        .create(&setup.server, "First", Kind::Manual, None)
        .await
        .unwrap();
    backups.wait(operation).await.unwrap();
    let zip = backups.file(&setup.server, &id);
    assert_eq!(
        entries(&zip),
        [
            "server.properties",
            "world/level.dat",
            "world/region/r.0.0.mca"
        ]
    );

    let dir = setup.paths.server(&setup.server);
    std::fs::write(dir.join("world").join("level.dat"), "changed").unwrap();
    std::fs::write(dir.join("world").join("new.dat"), "added since").unwrap();
    std::fs::write(dir.join("server.properties"), "motd=Changed\n").unwrap();
    let restore = backups.restore(&setup.server, &id, None).await.unwrap();
    backups.wait(restore).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("world").join("level.dat")).unwrap(),
        "level"
    );
    assert!(!dir.join("world").join("new.dat").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("server.properties")).unwrap(),
        "motd=Hi\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("mods").join("a.jar")).unwrap(),
        "mod"
    );
    let mut level = String::new();
    zip::ZipArchive::new(std::fs::File::open(&zip).unwrap())
        .unwrap()
        .by_name("world/level.dat")
        .unwrap()
        .read_to_string(&mut level)
        .unwrap();
    assert_eq!(level, "level");
}

#[tokio::test]
async fn each_kind_keeps_its_own_count() {
    let setup = setup().await;
    let backups = &setup.backups;
    records::set_schedule(
        &setup.db,
        &setup.server,
        records::Schedule {
            interval_hours: 24,
            keep: 3,
        },
    )
    .await
    .unwrap();
    for (kind, count) in [
        (Kind::Manual, 4),
        (Kind::Scheduled, 5),
        (Kind::PreUpdate, 4),
    ] {
        for _ in 0..count {
            let (_, operation) = backups
                .create(&setup.server, "x", kind, None)
                .await
                .unwrap();
            backups.wait(operation).await.unwrap();
        }
    }
    let kept = records::backups(&setup.db, &setup.server).await.unwrap();
    let count = |kind| kept.iter().filter(|backup| backup.kind == kind).count();
    assert_eq!(count(Kind::Manual), 4, "manual backups stay until deleted");
    assert_eq!(count(Kind::Scheduled), 3);
    assert_eq!(count(Kind::PreUpdate), 2);
    // Only the kept ones have files.
    let files = std::fs::read_dir(setup.paths.backups().join(&setup.server))
        .unwrap()
        .count();
    assert_eq!(files, kept.len());
}

#[tokio::test]
async fn a_schedule_backs_up_only_a_world_that_may_have_changed() {
    let setup = setup().await;
    let backups = &setup.backups;
    backups.queue_due().await.unwrap();
    assert!(
        records::backups(&setup.db, &setup.server)
            .await
            .unwrap()
            .is_empty(),
        "never ran"
    );

    servers_records::start_run(&setup.db, &setup.server, "run", now() - 100)
        .await
        .unwrap();
    servers_records::end_run(
        &setup.db,
        "run",
        &servers_records::Ended {
            at: now() - 50,
            outcome: servers_records::Outcome::Stopped,
            exit_code: Some(0),
            last_lines: None,
        },
    )
    .await
    .unwrap();
    backups.queue_due().await.unwrap();
    let queued = records::backups(&setup.db, &setup.server).await.unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].kind, Kind::Scheduled);
    let operation =
        records::operations(&setup.db, &setup.server).await.unwrap()[0].id;
    backups.wait(operation).await.unwrap();
    backups.queue_due().await.unwrap();
    assert_eq!(
        records::backups(&setup.db, &setup.server)
            .await
            .unwrap()
            .len(),
        1,
        "not again within the interval"
    );
}

#[tokio::test]
async fn a_failed_backup_leaves_nothing_behind() {
    let setup = setup().await;
    let backups = &setup.backups;
    // The server's folder is gone, so there is nothing to copy from.
    std::fs::remove_dir_all(setup.paths.server(&setup.server)).unwrap();
    let (id, operation) = backups
        .create(&setup.server, "x", Kind::Manual, None)
        .await
        .unwrap();
    assert!(backups.wait(operation).await.is_err());
    let backup = records::backup(&setup.db, &setup.server, &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(backup.state, records::State::Failed);
    let folder = setup.paths.backups().join(&setup.server);
    assert!(
        !folder.exists() || std::fs::read_dir(folder).unwrap().count() == 0
    );
}

#[tokio::test]
async fn a_download_link_works_once() {
    let setup = setup().await;
    let token =
        setup
            .backups
            .grant(Caller::Local, &setup.server, Target::World);
    let grant = setup.backups.take(&token).unwrap();
    assert_eq!(grant.target, Target::World);
    assert!(setup.backups.take(&token).is_none());
}
