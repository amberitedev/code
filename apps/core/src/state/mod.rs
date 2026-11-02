//! Copal's state in app-lib's layout (`packages/app-lib/src/state/`): a domain per folder, with
//! `model/` for plain types and `adapters/` for SQLite rows. Code moves here from the older
//! folders as it is restructured.

pub mod servers;
