/// Amberite Core library — exposes all modules so integration tests can import them.
///
/// Both `main.rs` (binary target) and `lib.rs` (library target) declare the same
/// modules. Rust compiles them as separate targets from the same source files.
/// Integration tests in `tests/` link against this library target.
/// Typed message layer. The relay store is wired into `presentation`; the
/// `Message`/`Distributor` half is a complete surface consumed by callers as the
/// post-and-distribute flows land.
#[allow(dead_code)]
pub mod api;
pub mod application;
pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod ports;
pub mod presentation;
