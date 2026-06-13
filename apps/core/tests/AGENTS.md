# tests

Core tests are integration-style HTTP tests against an in-process server with a temporary SQLite file database.

## Mental Model

`integration.rs` is the single integration test binary root. Feature test modules live under `integration/`.

`integration/common/mod.rs` owns `TestApp`, fixtures, and shared helpers. Pick the fixture based on what the test needs: normal dev-mode HTTP, mocked process spawning, paired auth state, or unpaired setup flow.

## File Relationships

- `integration.rs` declares the test modules.
- `integration/common/mod.rs` creates the app fixture, temp DB, HTTP client, and common request helpers.
- `integration/*.rs` contains feature-specific tests.
- Tests call HTTP endpoints through `reqwest`, so endpoint changes usually pair with `src/presentation/` and `src/application/` changes.
- Process lifecycle tests usually use the mock spawner from `src/infrastructure/process/mock_spawner.rs`.

## Running

From `apps/core/`:

```bash
cargo test --tests
```
