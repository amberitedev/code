//! TypeScript for the share, diff, history and edited-file types of Core's hosting routes,
//! generated with `ts-rs` into one file in `packages/api-client`, so a shape that differs between
//! Core and the App is a type error. Core's own routes are in `contracts::core`.
//!
//! `cargo test -p copal` fails when the file differs from what the structs generate. Regenerate it
//! with `pnpm --filter @amberite/core types`. The file is in `.prettierignore` so the formatter
//! leaves it exactly as generated.

use ts_rs::{Config, TS};

use super::{content, share};

const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/api-client/src/modules/copal/generated.ts"
);

/// Every type the App sees, in file order.
fn declarations() -> String {
    // Numbers in these types are unix seconds and small counts, which JSON gives as `number`.
    let cfg = Config::default().with_large_int("number");
    [
        content::EditedFiles::decl(&cfg),
        content::EditedFile::decl(&cfg),
        crate::state::servers::model::PendingChange::decl(&cfg),
        content::Decision::decl(&cfg),
        content::Choice::decl(&cfg),
        crate::event::GameVersionChange::decl(&cfg),
        share::SharedInstancePublishDiff::decl(&cfg),
        sharing_client::diff::SharedInstanceUpdateDiff::decl(&cfg),
        sharing_client::diff::SharedInstanceUpdateDiffType::decl(&cfg),
        sharing_client::history::SharedInstanceHistoryEntry::decl(&cfg),
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
