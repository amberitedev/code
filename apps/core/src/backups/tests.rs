//! The engine against a real in-memory database and a server folder on disk.

use std::io::{Read, Write};

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
            "copal-backup.json",
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

#[tokio::test]
async fn a_cancelled_operation_never_runs_and_a_claimed_one_cannot_be_cancelled()
 {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    folders::create(&paths, None).unwrap();
    let db = db::memory().await;
    // No engine runs here, so nothing claims the operations but the test.
    let servers = Servers::open(paths, db.clone()).await.unwrap();
    let server = servers.create("A").await.unwrap().id;
    let queue = |name: &'static str| {
        let db = db.clone();
        let server = server.clone();
        async move {
            let backup = records::Backup {
                id: Uuid::new_v4().to_string(),
                server_id: server,
                name: name.into(),
                kind: Kind::Manual,
                state: records::State::Queued,
                created_at: now(),
            };
            let id = records::queue_backup(&db, &backup, None).await.unwrap();
            records::operation(&db, id).await.unwrap().unwrap()
        }
    };

    let cancelled = queue("cancelled").await;
    assert!(records::cancel(&db, &server, cancelled.id).await.unwrap());
    assert!(!records::start(&db, &cancelled).await.unwrap());

    let claimed = queue("claimed").await;
    assert!(records::start(&db, &claimed).await.unwrap());
    assert!(!records::cancel(&db, &server, claimed.id).await.unwrap());
    assert_eq!(
        records::operation(&db, claimed.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        OpState::Running
    );
}

/// Make a backup of the set-up server and wait for it.
async fn backed_up(setup: &Setup) -> String {
    let (id, operation) = setup
        .backups
        .create(&setup.server, "x", Kind::Manual, None)
        .await
        .unwrap();
    setup.backups.wait(operation).await.unwrap();
    id
}

/// Whether a restore left its folder behind.
fn restore_left_behind(dir: &Path) -> bool {
    std::fs::read_dir(dir).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".copal-restore-")
    })
}

#[tokio::test]
async fn a_damaged_backup_changes_nothing_when_restored() {
    let setup = setup().await;
    let id = backed_up(&setup).await;
    // Damage the stored region file's bytes, so its checksum fails only once it is unpacked.
    let zip = setup.backups.file(&setup.server, &id);
    let mut bytes = std::fs::read(&zip).unwrap();
    let at = bytes
        .windows(6)
        .position(|window| window == b"chunks")
        .unwrap();
    bytes[at] = b'X';
    std::fs::write(&zip, bytes).unwrap();
    let dir = setup.paths.server(&setup.server);
    std::fs::write(dir.join("world").join("level.dat"), "now").unwrap();

    let restore = setup
        .backups
        .restore(&setup.server, &id, None)
        .await
        .unwrap();
    assert!(setup.backups.wait(restore).await.is_err());
    assert_eq!(
        std::fs::read_to_string(dir.join("world").join("level.dat")).unwrap(),
        "now"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("world/region/r.0.0.mca")).unwrap(),
        "chunks"
    );
    assert!(!restore_left_behind(&dir));
}

#[tokio::test]
async fn a_restore_replaces_only_the_worlds_and_settings_it_names() {
    let setup = setup().await;
    let dir = setup.paths.server(&setup.server);
    // The world is inside config, beside a config file a restore must not touch.
    std::fs::write(dir.join("server.properties"), "level-name=config/world\n")
        .unwrap();
    std::fs::create_dir_all(dir.join("config/world")).unwrap();
    std::fs::write(dir.join("config/world/level.dat"), "level").unwrap();
    std::fs::write(dir.join("config/other.toml"), "keep").unwrap();
    let id = backed_up(&setup).await;
    std::fs::write(dir.join("config/world/level.dat"), "changed").unwrap();
    std::fs::create_dir_all(dir.join("config/world_nether")).unwrap();
    std::fs::write(dir.join("config/world_nether/made-since"), "x").unwrap();

    let restore = setup
        .backups
        .restore(&setup.server, &id, None)
        .await
        .unwrap();
    setup.backups.wait(restore).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("config/world/level.dat")).unwrap(),
        "level"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("config/other.toml")).unwrap(),
        "keep"
    );
    // The backup speaks for the nether too, which did not exist then.
    assert!(!dir.join("config/world_nether").exists());
    // The world folder of the old name is not the backup's to touch.
    assert!(dir.join("world/level.dat").exists());
    assert!(!restore_left_behind(&dir));
}

#[tokio::test]
async fn a_backup_naming_more_than_it_may_is_refused_before_anything_changes() {
    let setup = setup().await;
    let id = backed_up(&setup).await;
    let zip = setup.backups.file(&setup.server, &id);
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&zip).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file(MANIFEST, options).unwrap();
    writer
        .write_all(br#"{"v":1,"worlds":["world"],"settings":[]}"#)
        .unwrap();
    writer.start_file("mods/a.jar", options).unwrap();
    writer.write_all(b"replaced").unwrap();
    writer.finish().unwrap();

    let restore = setup
        .backups
        .restore(&setup.server, &id, None)
        .await
        .unwrap();
    assert!(setup.backups.wait(restore).await.is_err());
    let dir = setup.paths.server(&setup.server);
    assert_eq!(
        std::fs::read_to_string(dir.join("mods").join("a.jar")).unwrap(),
        "mod"
    );
    assert!(dir.join("world").join("level.dat").exists());
}

#[tokio::test]
async fn files_are_listed_after_the_save_so_what_it_wrote_is_kept() {
    let setup = setup().await;
    let dir = setup.paths.server(&setup.server);
    let folder = Arc::new(Folder::open(&dir).unwrap());
    let destination = setup.paths.backups().join("after-save.zip");
    let flushed = dir.join("world").join("region").join("r.1.0.mca");
    copy(folder, &destination, |_| {}, async {
        // What `save-all flush` writes.
        std::fs::write(&flushed, "flushed").unwrap();
        Ok(None)
    })
    .await
    .unwrap();
    assert!(entries(&destination).contains(&"world/region/r.1.0.mca".into()));
}

#[tokio::test]
async fn a_copy_that_would_outlast_the_hold_fails_and_leaves_nothing() {
    let setup = setup().await;
    let folder =
        Arc::new(Folder::open(&setup.paths.server(&setup.server)).unwrap());
    let destination = setup.paths.backups().join("late.zip");
    let result = copy(folder, &destination, |_| {}, async {
        Ok(Some(Instant::now()))
    })
    .await;
    assert!(matches!(result, Err(Error::Conflict(_))));
    assert!(!destination.exists());
    assert!(!destination.with_extension("zip.part").exists());
}

#[tokio::test]
async fn a_file_that_grows_past_its_listed_size_fails_the_copy() {
    let setup = setup().await;
    let folder = Folder::open(&setup.paths.server(&setup.server)).unwrap();
    let mut listing = files(&folder).unwrap();
    // As if a mod grew the region file between the listing and the copy.
    for (name, size) in &mut listing.files {
        if name.ends_with(".mca") {
            *size -= 1;
        }
    }
    let path = setup.paths.backups().join("grown.zip");
    assert!(matches!(
        write_zip(&folder, &listing, &path, None, &|_| {}),
        Err(Error::Conflict(_))
    ));
}

#[test]
fn names_and_counts_a_restore_refuses_fail_the_backup() {
    // Fine on Linux, but a name Windows would change: a restore refuses it.
    assert!(restorable("world/bad:name").is_err());
    assert!(restorable("world/region/r.0.0.mca").is_ok());
    assert!(check_count(archive::MAX_ENTRIES - 1).is_ok());
    assert!(check_count(archive::MAX_ENTRIES).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn a_world_file_a_restore_would_refuse_fails_the_backup() {
    let setup = setup().await;
    let dir = setup.paths.server(&setup.server);
    std::fs::write(dir.join("world").join("bad:name"), "x").unwrap();
    let (id, operation) = setup
        .backups
        .create(&setup.server, "x", Kind::Manual, None)
        .await
        .unwrap();
    assert!(setup.backups.wait(operation).await.is_err());
    assert!(!setup.backups.file(&setup.server, &id).exists());
}
