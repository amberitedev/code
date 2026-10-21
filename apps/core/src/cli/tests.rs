//! Argument parsing, and one round trip per command group through a real local socket.

use std::sync::Arc;

use clap::{CommandFactory, Parser};

use super::{
    Cli, Client, Command, access::AccessCommand, backups::BackupsCommand,
    machine::UpdateCommand, servers::ServersCommand,
};
use crate::{
    access::{Access, keys::Keys},
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
            allow_downgrade: true
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
    let json: serde_json::Value =
        serde_json::from_str(&super::json_for(&error)).unwrap();
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

/// A Core with no backend serving the router on a local socket in a temporary folder, plus a
/// stand-in for step 5's backup list.
async fn core() -> (tempfile::TempDir, Arc<AppState>, Client) {
    use axum::{Json, routing::get};

    use crate::api::backups::{
        BackupQueueBackup, BackupStatus, BackupsQueueResponse,
    };

    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    folders::create(&paths, None).unwrap();
    let db = db::memory().await;
    db::core_id(&db).await.unwrap();
    let keys = Keys::load(db.clone(), reqwest::Client::new(), None)
        .await
        .unwrap();
    let state = Arc::new(AppState {
        servers: Servers::open(paths.clone(), db.clone()).await.unwrap(),
        access: Arc::new(Access::new(keys)),
        update: Arc::new(Updater::new(
            paths.clone(),
            db.clone(),
            reqwest::Client::new(),
            String::new(),
        )),
        db,
    });
    let backups = || async {
        Json(BackupsQueueResponse {
            active_operations: Vec::new(),
            backups: vec![BackupQueueBackup {
                id: uuid::Uuid::nil(),
                name: "Before the update".into(),
                created_at: chrono::Utc::now(),
                status: BackupStatus::Done,
                locked: false,
                automated: true,
                history: Vec::new(),
            }],
        })
    };
    let router = api::router(Arc::clone(&state))
        .route("/v1/servers/{id}/worlds/{wid}/backups-queue", get(backups));
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
    state.servers.create("Survival").await.unwrap();
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
