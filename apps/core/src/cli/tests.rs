//! Argument parsing, and one round trip per command through a real local socket.

use std::sync::Arc;

use clap::{CommandFactory, Parser};
use contracts::{
    ApiError, ErrorCode,
    core::{Channel, CheckState},
};

use super::{Cli, Client, Command, ui::Ui, update::UpdateArgs};
use crate::{
    access::CoreAccess,
    api::{self, CoreState},
    daemon::{accounts, folders, paths::Paths},
    db,
    servers::Servers,
    update::Updater,
};

fn parse(args: &str) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("copal").chain(args.split(' ')))
}

/// A test is no terminal: commands print nothing here and never ask.
fn ui() -> Ui {
    Ui::new(true)
}

#[test]
fn the_command_tree_is_valid() {
    Cli::command().debug_assert();
}

#[test]
fn only_the_essentials_are_offered() {
    let command = Cli::command();
    let offered: Vec<&str> = command
        .get_subcommands()
        .filter(|command| !command.is_hide_set())
        .map(clap::Command::get_name)
        .collect();
    assert_eq!(
        offered,
        [
            "status",
            "doctor",
            "pair",
            "unpair",
            "list",
            "start",
            "stop",
            "storage",
            "logs",
            "update",
            "uninstall",
        ]
    );
}

#[test]
fn arguments_parse_into_commands() {
    assert!(Cli::try_parse_from(["copal"]).unwrap().command.is_none());
    let cli = parse("--json update --yes --allow-downgrade").unwrap();
    assert!(cli.json);
    assert!(matches!(
        cli.command,
        Some(Command::Update(UpdateArgs {
            yes: true,
            allow_downgrade: true,
            retry: false,
            ..
        }))
    ));
    assert!(matches!(
        parse("update --channel beta").unwrap().command,
        Some(Command::Update(UpdateArgs {
            channel: Some(Channel::Beta),
            ..
        }))
    ));
    assert!(matches!(
        parse("stop --all --kill").unwrap().command,
        Some(Command::Stop {
            server: None,
            all: true,
            kill: true,
            yes: false,
        })
    ));
    let Some(Command::Start {
        server,
        accept_eula,
    }) = parse("start survival --accept-eula").unwrap().command
    else {
        panic!("not start");
    };
    assert_eq!(server.as_deref(), Some("survival"));
    assert!(accept_eula);
    assert!(matches!(
        parse("doctor --fix").unwrap().command,
        Some(Command::Doctor {
            fix: true,
            verbose: false,
        })
    ));
    // The json flag works after the command too.
    assert!(parse("list --json").unwrap().json);
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
    let error: color_eyre::Report = super::client::Refused(ApiError::new(
        ErrorCode::Conflict,
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
        "stop survival --all",
        "update --channel nightly",
        "update --check --yes",
        "update --rollback --retry",
        "uninstall --remove-data --keep-data",
        "logs -n many",
        // Servers are managed in the App.
        "servers list",
        "eula --accept",
        "install",
    ] {
        assert!(parse(args).is_err(), "{args} parsed");
    }
}

/// A Core with no backend serving the router on a local socket in a temporary folder.
async fn core() -> (tempfile::TempDir, Arc<CoreState>, Client) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    folders::create(&paths, None).unwrap();
    let engine = crate::daemon::start_engine(&paths, None).await.unwrap();
    let db = db::memory().await;
    let core_id = db::core_id(&db).await.unwrap();
    let keys = crate::access::keys::load(
        db.clone(),
        crate::network::http().build().unwrap(),
        None,
    )
    .await
    .unwrap();
    let servers = Servers::open(
        paths.clone(),
        db.clone(),
        Arc::clone(&engine),
        Some(std::net::Ipv4Addr::LOCALHOST.into()),
    )
    .await
    .unwrap();
    let events = crate::api::events::sender();
    let backups = crate::backups::Backups::open(
        paths.clone(),
        db.clone(),
        Arc::clone(&servers),
        events.clone(),
    )
    .await
    .unwrap();
    let state = Arc::new(CoreState {
        servers,
        engine,
        access: Arc::new(CoreAccess::new(db.clone(), keys)),
        backups,
        events,
        update: Arc::new(Updater::new(
            paths.clone(),
            db.clone(),
            crate::network::http().build().unwrap(),
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
        consoles: Arc::default(),
        ops: crate::api::ops::Ops::default(),
        db,
    });
    let router = api::router(Arc::clone(&state));
    let owner = accounts::current_id().unwrap();
    let socket = paths.clone();
    let mut serving = Box::pin(async move {
        crate::access::local::serve(&socket, &owner, router).await
    });
    // The first poll binds the socket, then waits for a connection.
    assert!(futures::poll!(serving.as_mut()).is_pending());
    tokio::spawn(serving);
    (dir, state, Client::at(paths))
}

#[tokio::test]
async fn status_and_update_settings_round_trip() {
    let (_dir, _state, client) = core().await;
    let ui = ui();
    let settings = super::update::run(
        &ui,
        &client,
        UpdateArgs {
            channel: Some(Channel::Alpha),
            ..no_update_flags()
        },
    )
    .await
    .unwrap();
    assert_eq!(settings.json["channel"], "alpha");
    let status = super::status::run(&ui, &client).await.unwrap();
    assert_eq!(status.json["update"]["channel"], "alpha");
    assert_eq!(status.json["pairing"]["paired"], false);
    assert_eq!(status.json["servers"], serde_json::json!([]));
    let home = super::home::run(&ui, &client).await.unwrap();
    assert_eq!(home.json["state"], "not_paired");
    assert_eq!(home.json["next"], "copal pair");
}

fn no_update_flags() -> UpdateArgs {
    UpdateArgs {
        check: false,
        yes: false,
        channel: None,
        auto: None,
        rollback: false,
        retry: false,
        allow_downgrade: false,
    }
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
async fn pairing_shows_the_code_then_who_it_is_paired_with() {
    let (_dir, state, client) = core().await;
    let ui = ui();
    // With no code yet, `copal pair` wakes the pairing loop, played here, and shows what it
    // gets. Outside a terminal it does not wait for the approval.
    let loop_state = Arc::clone(&state);
    let pairing_loop = tokio::spawn(async move {
        loop_state.access.new_code.notified().await;
        show_code(&loop_state.db, "abcd2345").await;
    });
    let pair = super::pairing::pair(&ui, &client, false).await.unwrap();
    assert_eq!(pair.json["code"], "ABCD-2345");
    pairing_loop.await.unwrap();
    // A code that still works is shown again rather than replaced.
    let pair = super::pairing::pair(&ui, &client, true).await.unwrap();
    assert_eq!(pair.json["code"], "ABCD-2345");

    sqlx::query(
        "UPDATE core SET owner_user_id = 'owner', backend_key = 'amc_test'",
    )
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO users (id, name, updated_at) VALUES ('owner', 'ilai', 0)",
    )
    .execute(&state.db)
    .await
    .unwrap();
    let pair = super::pairing::pair(&ui, &client, false).await.unwrap();
    assert_eq!(pair.json["paired"], true);
    assert_eq!(pair.json["owner"], "ilai");
    assert!(!pair.failed);
    let home = super::home::run(&ui, &client).await.unwrap();
    assert_eq!(home.json["state"], "running");

    // The way out asks first, and with nobody to ask it names the flag.
    let error = super::pairing::unpair(&ui, &client, false)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("--yes"), "{error}");
    let unpaired = super::pairing::unpair(&ui, &client, true).await.unwrap();
    assert_eq!(unpaired.json["paired"], false);
    // In Core itself too, not only in what the command printed.
    let owned: bool = sqlx::query_scalar(
        "SELECT owner_user_id IS NOT NULL FROM core WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert!(!owned);
}

#[tokio::test]
async fn servers_are_listed_started_and_stopped_by_name() {
    let (_dir, state, client) = core().await;
    let ui = ui();
    state.servers.create("Survival").await.unwrap();
    let list = super::servers::list(&ui, &client).await.unwrap();
    assert_eq!(list.json[0]["name"], "Survival");
    // With nobody to pick from a list, the answer names the servers.
    let error = super::servers::start(&ui, &client, None, false)
        .await
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("copal start <server>")
            && error.to_string().contains("Stopped: Survival"),
        "{error}"
    );
    // No server starts before the EULA is accepted, and the flag that accepts it is named.
    let error =
        super::servers::start(&ui, &client, Some("survival".into()), false)
            .await
            .err()
            .unwrap();
    assert!(error.to_string().contains("--accept-eula"), "{error}");
    // Refusing the start left it stopped.
    let list = super::servers::list(&ui, &client).await.unwrap();
    assert_eq!(list.json[0]["power_state"], "stopped");
    let stopped = super::servers::stop(
        &ui,
        &client,
        Some("survival".into()),
        false,
        false,
        false,
    )
    .await
    .unwrap();
    assert_eq!(stopped.json["power_state"], "stopped");
    let none = super::servers::stop(&ui, &client, None, true, false, false)
        .await
        .unwrap();
    assert_eq!(none.json, serde_json::json!([]));
}

#[tokio::test]
async fn storage_and_doctor_report_what_core_measured() {
    let (_dir, state, client) = core().await;
    let ui = ui();
    state.servers.create("Survival").await.unwrap();
    let storage = super::storage::run(&ui, &client, false).await.unwrap();
    assert_eq!(storage.json["servers"][0]["name"], "Survival");
    assert!(storage.json["total"].as_u64().unwrap() > 0);

    let doctor = super::doctor::run(&ui, &client, false, false)
        .await
        .unwrap();
    let state_of = |name: &str| {
        doctor.json["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["name"] == name)
            .unwrap_or_else(|| panic!("no {name} check"))["state"]
            .clone()
    };
    // This Core runs by hand, with no backend: said, and not a failure.
    assert_eq!(
        state_of("service"),
        serde_json::to_value(CheckState::Warn).unwrap()
    );
    assert_eq!(
        state_of("pairing"),
        serde_json::to_value(CheckState::Skip).unwrap()
    );
    assert_eq!(doctor.json["checks"][0]["group"], "computer");
}
