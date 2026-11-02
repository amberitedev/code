//! Copal's state in app-lib's layout (`packages/app-lib/src/state/`): a domain per folder, with
//! `model/` for plain types, `adapters/` for SQLite rows, files and the network, `domain/` for
//! pure rules, and `commands/` with one operation per file. The `api` routes call into it.

pub mod servers;
