# infrastructure/db/

SQLite implementations of the store ports.

## Files

| File | Implements |
|------|-----------|
| `instance_repo.rs` | `InstanceStore` for `InstanceRecord` |
| `modpack_repo.rs` | `ModpackStore` for `ModpackManifest` |

## Schema overview

### `instances` table
`id | name | game_version | loader | loader_version | port | memory_min | memory_max | java_version | status | data_dir | created_at | updated_at`

### `modpack_manifests` table
`id | instance_id | pack_name | pack_version | game_version | loader | loader_version | modrinth_project_id | modrinth_version_id | installed_at`

Full DDL lives in `migrations/`. Run automatically on startup via `sqlx::migrate!()`.

## Deserialization pattern

Each repo uses a private `*Row` struct with `#[derive(sqlx::FromRow)]`, then converts to the domain type via `TryFrom` (instances) or `From` (modpacks).

## Rules

- Use `sqlx::query` / `sqlx::query_as` — no raw string concatenation.
- All timestamps stored as RFC 3339 strings.
- `InstanceId` stored as lowercase hyphenated UUID string.
- `ModLoader` / `InstanceStatus` stored as lowercase strings, parsed via `FromStr`.
- `INSERT OR REPLACE` is acceptable for `modpack_manifests` (upsert semantics).
