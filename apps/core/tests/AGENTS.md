# tests/

Integration test suite for Amberite Core. **112 passing tests, 0 ignored.**

## Running tests

```bash
cargo test --tests                    # all integration + unit tests (Windows — avoids binary lock)
cargo test --tests -- --nocapture     # with stdout
cargo test --tests --test setup       # single test file
cargo test --tests -- --test-threads=1  # serial (avoids port conflicts on slow CI)
```

> On Windows, use `cargo test --tests` instead of `cargo test` to avoid the binary linker conflict when Core is already running.

## Structure

```
tests/
  common/mod.rs       — TestApp + fixtures (shared by all test files)
  health.rs           — 3 tests:  GET /health, /version, /java
  setup.rs            — 5 tests:  pairing flow end-to-end, lockout (SEC-01)
  instances.rs        — 9 tests:  CRUD, body validation, SEC-04 UUID check
  logs.rs             — 11 tests: list/read logs + crash-reports, path traversal (SEC-03)
  security.rs         — 9 tests:  JWT enforcement, lockout, path traversal, UUID injection
  lifecycle.rs        — 11 tests: start/stop/kill/command via MockSpawner
  edge_cases.rs       — 11 tests: concurrent creates, double-delete, field presence, auth guard
  instance_control.rs — 12 tests: invalid UUIDs, nonexistent IDs, offline-state 409s
  mods.rs             — 10 tests: mod list/upload/delete guards, invalid UUIDs
  properties_stats.rs — 14 tests: GET/PATCH properties, stats, diagnostics
```

## `TestApp` (`common/mod.rs`)

```rust
pub struct TestApp {
    pub base_url:     String,
    pub client:       reqwest::Client,
    pub pairing_code: Option<String>,   // Some(_) only for spawn_prod_unpaired()
    _data_dir:        tempfile::TempDir, // dropped = cleanup
}
```

### Constructors

| Constructor | dev_mode | core_config row | pairing_code field |
|-------------|----------|-----------------|--------------------|
| `spawn()` | `true` | none | `None` |
| `spawn_with_mock()` | `true` | none | `None` |
| `spawn_paired()` | `true` | pre-inserted | `None` |
| `spawn_prod_unpaired()` | `false` | none | `Some(generated)` |

Each creates a fresh `TempDir` + migrated SQLite file (not `:memory:`). Binds to `127.0.0.1:0` for an ephemeral port.

- Use `spawn()` for the majority of tests (no JWT needed).
- Use `spawn_with_mock()` for lifecycle tests — injects `MockSpawner` so `start_instance` doesn't try to spawn a real JVM.
- Use `spawn_prod_unpaired()` for auth/pairing tests — `pairing_code` is readable via `app.pairing_code`.

### Key helpers

```rust
app.url("/instances")                   // format base_url + path
default_create_body() -> Value          // valid POST /instances body
create_test_instance(app) -> String     // POST + return id
```

## Design decisions

- **File DB not `:memory:`** — ensures WAL mode and migration logic work identically to prod.
- **`dev_mode = true`** in most tests — avoids JWT complexity; auth tests use `spawn_prod_unpaired()`.
- **`reqwest::Client`** (not `axum::TestClient`) — tests actual HTTP, not in-process calls.
- **Background JAR download silently fails** in tests — DB record is still created; tests assert on the record, not the JAR.
- **`pairing_code` exposed on `TestApp`** — `spawn_prod_unpaired()` reads the generated code from `AppState` before the server starts, making full pairing flow tests possible without guessing.

## Open blocked tests

| Area | Blocker |
|------|---------|
| Modrinth mod install | Requires network or `wiremock` mock server |
| JWT validation (real token) | Needs a running JWKS server |
