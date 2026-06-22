/// Copal library — exposes all modules so integration tests can import them.
///
/// Both `main.rs` (binary target) and `lib.rs` (library target) declare the same
/// modules. Rust compiles them as separate targets from the same source files.
/// Integration tests in `tests/` link against this library target.
pub mod application;
pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod ports;
pub mod presentation;
