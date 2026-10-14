//! TypeScript for the Amberite-only routes, generated with `ts-rs` into one file in
//! `packages/api-client`, so a shape that differs between Core and the App is a type error.
//!
//! `cargo test -p copal` fails when the file is out of date. Regenerate it with
//! `cargo test -p copal -- --ignored write_typescript`.
//!
//! The repo's pre-commit hook reformats the file, so the check compares a hash of the raw output,
//! written in the file's header, instead of the text.

use sha2::{Digest, Sha256};
use ts_rs::{Config, TS};

use super::{access, activity, backend, error, sessions};

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
        backend::PublicKeys::decl(&cfg),
        backend::PublicKey::decl(&cfg),
        backend::StateReport::decl(&cfg),
        backend::AccessDirectory::decl(&cfg),
        backend::OwedCall::decl(&cfg),
    ]
    .map(|declaration| format!("export {declaration}\n"))
    .join("\n")
}

fn header(declarations: &str) -> String {
    let hash = hex::encode(Sha256::digest(declarations));
    format!(
        "// Generated from apps/core by ts-rs. Do not edit.\n\
         // Regenerate: cargo test -p copal -- --ignored write_typescript\n\
         // ts-rs output sha256: {hash}\n"
    )
}

#[test]
fn typescript_is_up_to_date() {
    let declarations = declarations();
    let written = std::fs::read_to_string(PATH).unwrap_or_default();
    assert!(
        written
            .replace("\r\n", "\n")
            .starts_with(&header(&declarations)),
        "{PATH} is out of date. Run: cargo test -p copal -- --ignored write_typescript"
    );
}

#[test]
#[ignore = "writes the generated file; run on purpose"]
fn write_typescript() {
    let declarations = declarations();
    let file = format!("{}\n{declarations}", header(&declarations));
    std::fs::create_dir_all(std::path::Path::new(PATH).parent().unwrap())
        .unwrap();
    std::fs::write(PATH, file).unwrap();
}
