# src/infrastructure/db — SQLite repository implementations

Concrete implementations of the `ports` traits using `sqlx` and SQLite. The connection pool is created by `connect()` in `mod.rs` and stored in `AppState`.

## File structure

```
db/
  mod.rs           — connect(): opens (or creates) the SQLite pool with WAL + 5s busy timeout
  instance_repo.rs — InstanceRepo: impl of InstanceStore
  java_repo.rs     — JavaRepo: impl of JavaStore
  modpack_repo.rs  — ModpackRepo: impl of ModpackStore
```

## mod.rs — connect()

Opens the DB at the given path. Key options:
- `create_if_missing(true)` — creates the file on first run.
- `journal_mode(Wal)` — WAL mode reduces write contention.
- `busy_timeout(5s)` — SQLite returns `BUSY` after 5s instead of spinning; prevents deadlocks under load.

## instance_repo.rs

`InstanceRow` is the flat `sqlx::FromRow` struct. `TryFrom<InstanceRow> for InstanceRecord` does the conversion, parsing loader and status from lowercase strings via their `FromStr` impls.

**BEH-09 timestamp parsing**: `parse_timestamp()` accepts both RFC 3339 (`"2024-01-01T12:00:00Z"`) and SQLite's native `CURRENT_TIMESTAMP` format (`"2024-01-01 12:00:00"`). Old rows inserted directly via SQL use the SQLite format; rows written by `InstanceRepo::create` use RFC 3339 explicitly.

`reset_transient_statuses`: single `UPDATE instances SET status = 'offline' WHERE status IN ('starting', 'stopping')`. Called once at startup to clean up after an unclean shutdown.

## java_repo.rs

`sync_all` uses `INSERT OR REPLACE` — upserts by `version` (the major version integer, e.g. `21`). If multiple installs share the same version, only one is stored.

## modpack_repo.rs

`save` upserts the manifest row — only one manifest per instance (`UNIQUE(instance_id)` in the schema). Calling `save` a second time for the same instance replaces the previous manifest.

## Gotchas

- **`mod_service` bypasses these repos entirely**: The `mods` table is queried with raw `sqlx::query` directly against `state.pool` — it has no repo class and is not backed by a port trait.
- **Adding `sqlx::query!` macros requires `cargo sqlx prepare`**: `SQLX_OFFLINE=true` (set in `.cargo/config.toml`) means the compiler uses a cached metadata file. Raw `sqlx::query` (non-macro) is unaffected.
- **`StoreError::NotFound` holds a `String`**: The string is the UUID text. Callers that need an `InstanceError::NotFound(InstanceId)` convert it manually.
