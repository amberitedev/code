# infrastructure/db/

SQLite implementations of the store ports using `sqlx`.

## Files

| File | Implements |
|------|-----------|
| `instance_repo.rs` | `InstanceStore` → `InstanceRepo` |
| `modpack_repo.rs` | `ModpackStore` → `ModpackRepo` |
| `mod.rs` | Re-exports + `connect(path)` helper |

## `InstanceRepo`

Wraps a `SqlitePool`. Constructed in `AppState::new()` and stored as `Arc<dyn InstanceStore>`.

### Deserialization pattern

Private `InstanceRow` struct derives `sqlx::FromRow`. All columns are primitives (strings, i64).  
`TryFrom<InstanceRow> for InstanceRecord` parses `loader`, `status`, timestamps, UUID via `FromStr`.

### SQL schema (`instances` table)

```
id TEXT PK | name TEXT | game_version TEXT | loader TEXT | loader_version TEXT
port INTEGER | memory_min INTEGER | memory_max INTEGER | java_version INTEGER
status TEXT | data_dir TEXT | created_at TEXT | updated_at TEXT
```

All timestamps stored as RFC 3339 strings. `id` is lowercase hyphenated UUID. `loader`/`status` are lowercase strings parsed via `FromStr`.

### Methods

| Method | SQL |
|--------|-----|
| `create` | `INSERT INTO instances (...) VALUES (...)` |
| `get` | `SELECT * FROM instances WHERE id = ?` |
| `list` | `SELECT * FROM instances ORDER BY created_at` |
| `list_by_status` | `SELECT * FROM instances WHERE status = ?` |
| `update_status` | `UPDATE instances SET status = ?, updated_at = ? WHERE id = ?` |
| `delete` | `DELETE FROM instances WHERE id = ?` |

## `ModpackRepo`

Wraps a `SqlitePool`. Stored as `Arc<dyn ModpackStore>`.

### SQL schema (`modpack_manifests` table)

```
id TEXT PK | instance_id TEXT | pack_name TEXT | pack_version TEXT
game_version TEXT | loader TEXT | loader_version TEXT
modrinth_project_id TEXT | modrinth_version_id TEXT | installed_at TEXT
```

Uses `INSERT OR REPLACE` for upsert semantics.

## `connect(path: &Path)` helper (`mod.rs`)

Opens a `SqlitePool` with `create_if_missing(true)`. Used in `main.rs` and `TestApp`.

## Migrations

`sqlx::migrate!("./migrations")` is called on startup and in `TestApp`. Files are embedded at **compile time**. After adding a new `.sql` file, run `cargo clean -p amberite-core` to force re-embedding.

Current migrations:
- `001_init.sql` — initial schema
- `002_full_rewrite.sql` — complete table redesign
- `003_alter_instances.sql` — add game_version, loader, port, memory, status, data_dir columns
- `004_mods.sql` — `mods` + `java_installations` tables
- `005_fix_instances.sql` — add missing `loader_version`, `java_version`, `updated_at` columns

## Rules

- Use `sqlx::query` / `sqlx::query_as` only — no string concatenation or format! in SQL.
- No `sqlx::query!` macros (no `.sqlx/` offline cache checked in).
- `SQLX_OFFLINE=true` is set in `.cargo/config.toml`; this disables compile-time type checking for query macros (which we don't use).
