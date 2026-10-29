//! Argument parsing, and one round trip per command group through a real local socket.

use std::sync::Arc;

use clap::{CommandFactory, Parser};

use super::{
    Cli, Client, Command, access::AccessCommand, backups::BackupsCommand,
    machine::UpdateCommand, servers::ServersCommand,
};
use crate::{
    access::Access,
    api::{self, AppState},
    daemon::{accounts, folders, paths::Paths},
    db,
    servers::{
        Servers,
        restarts::{Schedule, Weekday},
    },
    update::{Updater, release::Channel},
};

fn parse(args: &str) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("copal").chain(args.split(' ')))
}

#[test]
fn the_command_tree_is_valid() {
    Cli::command().debug_assert();
}

#[test]
fn arguments_parse_into_commands() {
    let cli = parse("--json update apply --allow-downgrade").unwrap();
    assert!(cli.json);
    assert!(matches!(
        cli.command,
        Command::Update(UpdateCommand::Apply {
            allow_downgrade: true,
            retry: false,
        })
    ));
    assert!(matches!(
        parse("update apply --retry").unwrap().command,
        Command::Update(UpdateCommand::Apply {
            allow_downgrade: false,
            retry: true,
        })
    ));
    assert!(matches!(
        parse("update channel beta").unwrap().command,
        Command::Update(UpdateCommand::Channel {
            channel: Some(Channel::Beta)
        })
    ));
    let Command::Servers(ServersCommand::Restarts { at, on, smart, .. }) =
        parse("servers restarts survival --at 04:30 --on sunday --smart 12")
            .unwrap()
            .command
    else {
        panic!("not restarts");
    };
    assert_eq!(at.map(String::from).as_deref(), Some("04:30"));
    assert_eq!(on, Some(Weekday::Sunday));
    assert_eq!(smart, Some(12));
    let Command::Servers(ServersCommand::Console(
        super::servers::ConsoleCommand::Send { command, .. },
    )) = parse("servers console send survival say hello there")
        .unwrap()
        .command
    else {
        panic!("not console send");
    };
    assert_eq!(command, ["say", "hello", "there"]);
    assert!(matches!(parse("pair").unwrap().command, Command::Pair));
    // The json flag works after the command too.
    assert!(parse("backups list survival --json").unwrap().json);
}

#[test]
fn console_text_cannot_drive_the_terminal() {
    let hostile = "ok\u{1b}[2J\u{1b}]0;owned\u{7}\r\u{9b}31m\tdone\nnext";
    let shown = super::safe(hostile);
    assert!(
        !shown
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    );
    assert_eq!(
        shown,
        "ok\\u{1b}[2J\\u{1b}]0;owned\\u{7}\\u{d}\\u{9b}31m\tdone\nnext"
    );
}

#[test]
fn errors_print_as_json_in_cores_shape() {
    let error: color_eyre::Report =
        super::client::Refused(crate::api::error::ApiError::new(
            crate::api::error::ErrorCode::Conflict,
            "the server is not running",
        ))
        .into();
    let json = super::json_for(&error);
    assert_eq!(json["error"], "conflict");
    assert_eq!(json["description"], "the server is not running");
}

#[test]
fn bad_arguments_are_refused() {
    for args in [
        "servers java survival",
        "servers java survival /usr/bin/java --managed",
        "servers restarts survival --on sunday",
        "servers restarts survival --at 25:00",
        "servers restarts survival --at 04:00 --no-schedule",
        "access role survival friend admin",
        "update channel nightly",
        "backups schedule survival --every 6 --off",
        "servers console send survival",
    ] {
        assert!(parse(args).is_err(), "{args} parsed");
    }
}

/// A Core with no backend serving the router on a local socket in a temporary folder.
async fn core() -> (tempfile::TempDir, Arc<AppState>, Client) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    folders::create(&paths, None).unwrap();
    let db = db::memory().await;
    let core_id = db::core_id(&db).await.unwrap();
    let keys =
        crate::access::keys::load(db.clone(), reqwest::Client::new(), None)
            .await
            .unwrap();
    let servers = Servers::open(paths.clone(), db.clone()).await.unwrap();
    let backups = crate::backups::Backups::open(
        paths.clone(),
        db.clone(),
        Arc::clone(&servers),
    )
    .await
    .unwrap();
    let content = crate::content::Content::open(
        paths.clone(),
        db.clone(),
        Arc::clone(&servers),
        Arc::clone(&backups),
    )
    .unwrap();
    let state = Arc::new(AppState {
        servers,
        access: Arc::new(Access::new(keys)),
        content,
        backups,
        uploads: api::files::Uploads::default(),
        update: Arc::new(Updater::new(
            paths.clone(),
            db.clone(),
            reqwest::Client::new(),
            String::new(),
        )),
        network: Arc::new(crate::network::forward::Network::off(db.clone())),
        storage: Arc::new(crate::storage::Storage::new(
            db.clone(),
            paths.clone(),
            core_id,
            None,
            false,
            Arc::new(tokio::sync::Notify::new()),
        )),
        db,
    });
    let router = api::router(Arc::clone(&state));
    let owner = accounts::current_id().unwrap();
    let socket = paths.clone();
    tokio::spawn(async move {
        crate::access::local::serve(&socket, &owner, router).await
    });
    let client = Client::at(paths);
    for _ in 0..50 {
        if client.get::<serde_json::Value>("/health").await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    (dir, state, client)
}

#[tokio::test]
async fn core_commands_round_trip() {
    let (_dir, _state, client) = core().await;
    let eula = super::machine::eula(&client, false).await.unwrap();
    assert!(eula.json["accepted_at"].is_null());
    let eula = super::machine::eula(&client, true).await.unwrap();
    assert!(eula.json["accepted_at"].is_i64());
    let status = super::machine::update(
        &client,
        UpdateCommand::Channel {
            channel: Some(Channel::Alpha),
        },
    )
    .await
    .unwrap();
    assert_eq!(status.json["channel"], "alpha");
    let status = super::machine::status(&client).await.unwrap();
    assert_eq!(status.json["update"]["channel"], "alpha");
    assert!(status.text.contains("No servers yet"), "{}", status.text);
}

/// Store `code` as the one Core shows, as the pairing loop does once Amberite gives it one.
async fn show_code(db: &sqlx::SqlitePool, code: &str) {
    sqlx::query(
        "INSERT INTO pairing (id, code, key, expires_at, poll_interval) \
         VALUES (1, ?, 'amc_test', unixepoch() + 600, 5) \
         ON CONFLICT (id) DO UPDATE SET code = excluded.code",
    )
    .bind(code)
    .execute(db)
    .await
    .unwrap();
}

#[tokio::test]
async fn an_unpaired_core_shows_its_code() {
    let (_dir, state, client) = core().await;
    let status = super::machine::status(&client).await.unwrap();
    assert!(
        status.text.contains("still getting a code"),
        "{}",
        status.text
    );
    show_code(&state.db, "mzuxd576").await;
    let status = super::machine::status(&client).await.unwrap();
    assert!(status.text.contains("enter MZUX-D576"), "{}", status.text);
    assert!(status.text.contains("copal pair"), "{}", status.text);
    assert_eq!(status.json["pairing"]["code"], "MZUX-D576");

    // `copal pair` wakes the pairing loop, played here, and prints the code it gets.
    let loop_state = Arc::clone(&state);
    let pairing_loop = tokio::spawn(async move {
        loop_state.access.new_code.notified().await;
        show_code(&loop_state.db, "abcd2345").await;
    });
    let pair = super::machine::pair(&client).await.unwrap();
    assert!(pair.text.contains("enter ABCD-2345"), "{}", pair.text);
    pairing_loop.await.unwrap();

    // Once paired, both say so.
    sqlx::query(
        "UPDATE core SET owner_user_id = 'owner', backend_key = 'amc_test'",
    )
    .execute(&state.db)
    .await
    .unwrap();
    let status = super::machine::status(&client).await.unwrap();
    assert!(!status.text.contains("not paired"), "{}", status.text);
    let error = super::machine::pair(&client).await.err().unwrap();
    assert!(error.to_string().contains("already paired"), "{error}");
}

#[tokio::test]
async fn server_commands_round_trip() {
    let (_dir, state, client) = core().await;
    let server = state.servers.create("Survival").await.unwrap();
    let memory = super::servers::run(
        &client,
        parse("servers memory survival 3072").unwrap_servers(),
    )
    .await
    .unwrap();
    assert_eq!(memory.json["memory_mb"], 3072);
    let restarts = super::servers::run(
        &client,
        parse("servers restarts survival --crash off --at 05:00 --on monday")
            .unwrap_servers(),
    )
    .await
    .unwrap();
    assert_eq!(
        crate::servers::restarts::get(&state.db, &server.id)
            .await
            .unwrap()
            .schedule,
        Some(Schedule::Weekly {
            on: Weekday::Monday,
            at: "05:00".parse().unwrap()
        })
    );
    assert!(restarts.text.contains("after a crash: off"));
    let error = super::servers::run(
        &client,
        parse("servers start survival").unwrap_servers(),
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("EULA"), "{error}");
    // Core says why it did not pass a command on, and the command fails.
    let error = super::servers::run(
        &client,
        parse("servers console send survival say hi").unwrap_servers(),
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("not running"), "{error}");
}

#[tokio::test]
async fn access_commands_round_trip() {
    let (_dir, state, client) = core().await;
    let server = state.servers.create("Survival").await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, name, updated_at) VALUES ('u1', 'Friend', 0)",
    )
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO access (server_id, user_id, role, added_at) VALUES (?, 'u1', 'viewer', 0)")
        .bind(&server.id)
        .execute(&state.db)
        .await
        .unwrap();
    let changed = super::access::run(
        &client,
        AccessCommand::Role {
            server: "survival".into(),
            user: "friend".into(),
            role: api::access::Role::Editor,
        },
    )
    .await
    .unwrap();
    assert_eq!(changed.json["role"], "editor");
    let list = super::access::run(
        &client,
        AccessCommand::List {
            server: "survival".into(),
        },
    )
    .await
    .unwrap();
    assert!(list.text.contains("editor Friend"), "{}", list.text);
    super::access::run(
        &client,
        AccessCommand::Remove {
            server: "survival".into(),
            user: "u1".into(),
        },
    )
    .await
    .unwrap();
    let sessions =
        super::access::run(&client, AccessCommand::Sessions { user: None })
            .await
            .unwrap();
    assert_eq!(sessions.text, "No sessions");
}

#[tokio::test]
async fn backup_commands_read_what_the_backup_routes_send() {
    let (_dir, state, client) = core().await;
    let server = state.servers.create("Survival").await.unwrap().id;
    // A pre-update backup that finished, as the engine records one.
    sqlx::query(
        "INSERT INTO backups (id, server_id, name, kind, state, created_at) \
         VALUES (?, ?, 'Before the update', 'pre_update', 'done', 0)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&server)
    .execute(&state.db)
    .await
    .unwrap();
    let list = super::backups::run(
        &client,
        BackupsCommand::List {
            server: "survival".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        list.text.contains("Before the update (scheduled)"),
        "{}",
        list.text
    );
}

trait UnwrapServers {
    fn unwrap_servers(self) -> ServersCommand;
}

impl UnwrapServers for Result<Cli, clap::Error> {
    fn unwrap_servers(self) -> ServersCommand {
        match self.unwrap().command {
            Command::Servers(command) => command,
            other => panic!("{other:?} is not a servers command"),
        }
    }
}

#[tokio::test]
async fn failed_backups_and_installs_say_why() {
    let (_dir, state, client) = core().await;
    let server = state.servers.create("Survival").await.unwrap().id;
    let backup = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO backups (id, server_id, name, kind, state, created_at) \
         VALUES (?, ?, 'Nightly', 'scheduled', 'failed', 0)",
    )
    .bind(&backup)
    .bind(&server)
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO backup_operations (backup_id, server_id, kind, state, queued_at, error) \
         VALUES (?, ?, 'create', 'failed', 0, 'Core may not read world/level.dat')",
    )
    .bind(&backup)
    .bind(&server)
    .execute(&state.db)
    .await
    .unwrap();
    let list = super::backups::run(
        &client,
        BackupsCommand::List {
            server: "survival".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        list.text
            .contains("create failed: Core may not read world/level.dat"),
        "{}",
        list.text
    );

    sqlx::query(
        "INSERT INTO packs (server_id, name, version_number, sha1, error) \
         VALUES (?, 'Pack', '1', 'abc', 'the pack is for Minecraft 26.2')",
    )
    .bind(&server)
    .execute(&state.db)
    .await
    .unwrap();
    let info = super::servers::run(
        &client,
        ServersCommand::Info {
            server: "survival".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        info.text
            .contains("last install failed: the pack is for Minecraft 26.2"),
        "{}",
        info.text
    );
}
