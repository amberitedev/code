# migrations/

SQLx migration files. Applied automatically on startup via `sqlx::migrate!("./migrations")` and by `cargo run -- migrate`.

## Compile-time embedding

`sqlx::migrate!()` embeds SQL files at compile time. After adding or editing a migration, run `cargo clean -p amberite-core` before building — otherwise the old embedded SQL is used.

## Files

| File | Purpose |
|------|---------|
| `001_init.sql` | Legacy initial schema — `users`, bare `instances`, `paseto_key`, `events` (pre-rewrite, largely superseded) |
| `002_full_rewrite.sql` | Full rewrite schema: `java_installations`, full `instances`, `modpack_manifests`, `core_config` |
| `003_alter_instances.sql` | Adds missing columns to `instances` (`game_version`, `loader`, `port`, `memory_min`, `memory_max`, `status`, `data_dir`) because migration 002's `CREATE TABLE IF NOT EXISTS` was silently skipped when 001 had already created the table |
| `004_mods.sql` | Adds `mods` table |
| `005_fix_instances.sql` | Adds `loader_version TEXT`, `java_version INTEGER`, `updated_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00+00:00'` — columns missed in migration 003 |
| `006_fix_nulls.sql` | Back-fills NULL sentinels in `instances` for rows that pre-date migration 003 (MIGS-01): sets `game_version='unknown'`, `loader='vanilla'`, `port=25565`, `memory_min=1024`, `memory_max=4096`, `data_dir=''` |
| `007_backups.sql` | Adds `backups` table (per-instance backup records with lock + trigger fields) and `backup_schedules` table (one row per instance, cron + retain_count) |

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
id                   TEXT PRIMARY KEY
instance_id          TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE
filename             TEXT NOT NULL
display_name         TEXT
modrinth_project_id  TEXT
modrinth_version_id  TEXT
version_number       TEXT
client_side          TEXT
server_side          TEXT
sha512               TEXT
enabled              INTEGER NOT NULL DEFAULT 1
installed_at         TEXT NOT NULL
UNIQUE(instance_id, filename)
```

### `java_installations`
```sql
version   INTEGER PRIMARY KEY
path      TEXT    NOT NULL
```

### `backups`
```sql
id          TEXT    PRIMARY KEY
instance_id TEXT    NOT NULL REFERENCES instances(id) ON DELETE CASCADE
name        TEXT    NOT NULL
size_bytes  INTEGER NOT NULL DEFAULT 0
locked      INTEGER NOT NULL DEFAULT 0
trigger     TEXT    NOT NULL DEFAULT 'manual'
created_at  TEXT    NOT NULL
```

### `backup_schedules`
```sql
instance_id  TEXT    PRIMARY KEY REFERENCES instances(id) ON DELETE CASCADE
enabled      INTEGER NOT NULL DEFAULT 0
cron         TEXT    NOT NULL DEFAULT '0 4 * * *'
retain_count INTEGER NOT NULL DEFAULT 5
```

## Rules

- Never modify existing migration files — always add a new numbered file.
- All timestamps are RFC 3339 strings stored as `TEXT`.
