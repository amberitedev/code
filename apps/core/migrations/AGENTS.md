# migrations/

SQLx migration files. Applied automatically on startup via `sqlx::migrate!("./migrations")` and by `cargo run -- migrate`.

## Critical: compile-time embedding

`sqlx::migrate!()` is a **proc macro** that embeds SQL files at compile time. After adding or modifying a migration file, you must force a recompile:

```bash
cargo clean -p amberite-core
cargo build
```

Otherwise the old embedded SQL is used and your new migration won't run.

## Files

| File | Purpose |
|------|---------|
| `001_init.sql` | Initial schema — `instances`, `core_config` tables |
| `002_full_rewrite.sql` | Complete table redesign |
| `003_alter_instances.sql` | Add `game_version`, `loader`, `port`, `memory_min`, `memory_max`, `status`, `data_dir` columns |
| `004_mods.sql` | Add `mods` + `java_installations` tables |
| `005_fix_instances.sql` | Add missing `loader_version TEXT`, `java_version INTEGER`, `updated_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00+00:00'` |

## Current effective schema

### `instances`
```sql
id TEXT PRIMARY KEY
name TEXT NOT NULL
game_version TEXT NOT NULL
loader TEXT NOT NULL
loader_version TEXT
port INTEGER NOT NULL
memory_min INTEGER NOT NULL
memory_max INTEGER NOT NULL
java_version INTEGER
status TEXT NOT NULL DEFAULT 'offline'
data_dir TEXT NOT NULL
created_at TEXT NOT NULL
updated_at TEXT NOT NULL
```

### `core_config`
```sql
id INTEGER PRIMARY KEY
supabase_url TEXT NOT NULL
owner_user_id TEXT NOT NULL
paired_at TEXT NOT NULL
```

### `modpack_manifests`
```sql
id TEXT PRIMARY KEY
instance_id TEXT NOT NULL
pack_name TEXT NOT NULL
pack_version TEXT NOT NULL
game_version TEXT NOT NULL
loader TEXT NOT NULL
loader_version TEXT
modrinth_project_id TEXT
modrinth_version_id TEXT
installed_at TEXT NOT NULL
```

### `mods`
```sql
instance_id TEXT NOT NULL
filename TEXT NOT NULL
enabled INTEGER NOT NULL DEFAULT 1
modrinth_project_id TEXT
modrinth_version_id TEXT
PRIMARY KEY (instance_id, filename)
```

### `java_installations`
```sql
path TEXT PRIMARY KEY
major_version INTEGER NOT NULL
full_version TEXT NOT NULL
```

## Rules

- Never modify existing migration files — always add a new numbered file.
- Migrations are applied in filename order (`001_` before `002_` etc.).
- All timestamps are RFC 3339 strings stored as `TEXT`.
- Use `ALTER TABLE ... ADD COLUMN` with a default for non-nullable new columns.
- After adding a new migration, run `cargo clean -p amberite-core` before building.
