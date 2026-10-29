//! TypeScript for the Amberite-only routes, generated with `ts-rs` into one file in
//! `packages/api-client`, so a shape that differs between Core and the App is a type error.
//!
//! `cargo test -p copal` fails when the file differs from what the structs generate. Regenerate it
//! with `pnpm --filter @amberite/core types`. The file is in `.prettierignore` so the formatter
//! leaves it exactly as generated.

use ts_rs::{Config, TS};

use super::{
    access, activity, backend, console, content, error, eula, events, insight,
    logs, pairing, performance, sessions, startup, storage, update,
};

const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/api-client/src/modules/copal/generated.ts"
);

/// Every type the App sees, in file order.
fn declarations() -> String {
    // Numbers in these types are unix seconds and small counts, which JSON gives as `number`.
    let cfg = Config::default().with_large_int("number");
    [
        error::ApiError::decl(&cfg),
        error::ErrorCode::decl(&cfg),
        access::Role::decl(&cfg),
        access::User::decl(&cfg),
        access::Member::decl(&cfg),
        access::ServerAccess::decl(&cfg),
        access::Invite::decl(&cfg),
        access::ChangeRole::decl(&cfg),
        access::MyAccess::decl(&cfg),
        access::ServerRole::decl(&cfg),
        sessions::SignIn::decl(&cfg),
        sessions::SignedIn::decl(&cfg),
        sessions::Session::decl(&cfg),
        activity::ActivityQuery::decl(&cfg),
        activity::ActivityPage::decl(&cfg),
        activity::ActivityEntry::decl(&cfg),
        activity::Action::decl(&cfg),
        backend::TicketClaims::decl(&cfg),
        sharing_storage::keys::PublicKeys::decl(&cfg),
        sharing_storage::keys::PublicKey::decl(&cfg),
        backend::StateReport::decl(&cfg),
        backend::VisibleServer::decl(&cfg),
        backend::AccessDirectory::decl(&cfg),
        backend::Grant::decl(&cfg),
        backend::Remove::decl(&cfg),
        backend::OwedCall::decl(&cfg),
        backend::StorageReport::decl(&cfg),
        sharing_storage::local::HeldVersion::decl(&cfg),
        backend::StorageAnswer::decl(&cfg),
        backend::PullOrder::decl(&cfg),
        content::EditedFiles::decl(&cfg),
        content::EditedFile::decl(&cfg),
        content::PendingChange::decl(&cfg),
        content::Decision::decl(&cfg),
        content::Choice::decl(&cfg),
        startup::JavaPath::decl(&cfg),
        events::GameVersionChange::decl(&cfg),
        update::UpdateStatus::decl(&cfg),
        crate::update::versions::FailedTrial::decl(&cfg),
        crate::update::release::Channel::decl(&cfg),
        update::UpdateSettings::decl(&cfg),
        update::ApplyUpdate::decl(&cfg),
        update::Restarting::decl(&cfg),
        storage::StorageSettings::decl(&cfg),
        storage::StorageStatus::decl(&cfg),
        eula::Eula::decl(&cfg),
        pairing::PairingStatus::decl(&cfg),
        logs::LogsQuery::decl(&cfg),
        logs::CoreLogs::decl(&cfg),
        performance::Performance::decl(&cfg),
        performance::Memory::decl(&cfg),
        console::SendCommand::decl(&cfg),
        performance::JvmFlags::decl(&cfg),
        insight::ServerInsight::decl(&cfg),
        insight::Crash::decl(&cfg),
        insight::ReachabilityReport::decl(&cfg),
        insight::Reach::decl(&cfg),
        insight::ReachabilityCheck::decl(&cfg),
        insight::Machine::decl(&cfg),
        crate::servers::restarts::Restarts::decl(&cfg),
        crate::servers::restarts::Schedule::decl(&cfg),
        crate::servers::restarts::Weekday::decl(&cfg),
    ]
    .map(|declaration| format!("export {declaration}\n"))
    .join("\n")
}

/// The whole file, as `write_typescript` writes it.
fn file() -> String {
    format!(
        "// Generated from apps/core by ts-rs. Do not edit.\n\
         // Regenerate: pnpm --filter @amberite/core types\n\n{}",
        declarations()
    )
}

#[test]
fn typescript_is_up_to_date() {
    let written = std::fs::read_to_string(PATH).unwrap_or_default();
    assert!(
        written.replace("\r\n", "\n") == file(),
        "{PATH} is out of date. Run: pnpm --filter @amberite/core types"
    );
}

#[test]
#[ignore = "writes the generated file; run on purpose"]
fn write_typescript() {
    std::fs::create_dir_all(std::path::Path::new(PATH).parent().unwrap())
        .unwrap();
    std::fs::write(PATH, file()).unwrap();
}
