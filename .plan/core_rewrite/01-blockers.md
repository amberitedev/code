# Part 1 — Blockers

These 6 issues prevent the Core from compiling or working correctly at runtime.
Fix all of them before touching any new features.

---

## B1 — Fix Serde Version Pin

**File:** `apps/core/Cargo.toml`

**Problem:** `serde = "=1.0.219"` uses a strict exact pin. `serde_json = "1.0.145"`
transitively requires serde `^1.0.220`. Cargo cannot resolve the conflict.
The project does not compile at all.

**Fix:** Change the serde line to `serde = "^1.0.220"`. Run `cargo check` to confirm
the dep tree resolves cleanly.

---

## B2 — Fix Migration 002 Silent Skip

**Files:** `migrations/001_init.sql`, `migrations/002_full_rewrite.sql`

**Problem:** Migration `001` creates the `instances` table with minimal columns.
Migration `002` runs `CREATE TABLE IF NOT EXISTS instances` with the full schema —
but because the table already exists, SQLite silently skips the entire statement.
The extended columns are never added:

- `game_version TEXT`
- `loader TEXT`
- `port INTEGER`
- `memory_min INTEGER`
- `memory_max INTEGER`
- `status TEXT DEFAULT 'stopped'`
- `data_dir TEXT`

Every query that references these columns crashes at runtime with a "no such column"
error. This affects nearly every service function.

**Fix:** Create `migrations/003_alter_instances.sql`. Use `ALTER TABLE instances ADD COLUMN`
for each missing column. Each ALTER must be safe to re-run:

```sql
ALTER TABLE instances ADD COLUMN IF NOT EXISTS game_version TEXT;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS loader TEXT;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS port INTEGER;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS memory_min INTEGER;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS memory_max INTEGER;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS status TEXT DEFAULT 'stopped';
ALTER TABLE instances ADD COLUMN IF NOT EXISTS data_dir TEXT;
```

**Do NOT modify 001 or 002** — they are history. Always add forward-only migrations.

---

## B3 — Wire Repos Into AppState

**Files:** `application/state.rs`, `main.rs`, `application/instance_service.rs`,
`application/modpack_service.rs`

**Problem:** `AppState` holds only `pool: SqlitePool`. `InstanceRepo` and `ModpackRepo`
are fully implemented in `infrastructure/db/` but are never instantiated. All service
code writes raw SQL inline, bypassing the repo layer entirely.

**Fix — Step 1:** Add two fields to `AppState`:

```rust
instance_store: Arc<dyn InstanceStore + Send + Sync>,
modpack_store: Arc<dyn ModpackStore + Send + Sync>,
```

**Fix — Step 2:** In `main.rs` at startup, create and inject:

```rust
let instance_repo = Arc::new(InstanceRepo::new(pool.clone()));
let modpack_repo = Arc::new(ModpackRepo::new(pool.clone()));
```

**Fix — Step 3:** Replace every raw SQL block in `instance_service.rs` and
`modpack_service.rs` with calls to the store methods
(e.g., `state.instance_store.save(instance).await?`).

**Don't:** Create a new repo or new trait. The existing ones are correct — just wire them.

---

## B4 — Write server.properties on Instance Creation

**Files:** `application/instance_service.rs`,
`infrastructure/minecraft/server_properties.rs`

**Problem:** `create_instance()` creates the instance directory and saves the DB row, but
never calls `write_initial_properties()`. New servers start without a `server.properties`
file. Minecraft refuses to start: "You need to agree to the EULA."

**Fix:** After the instance directory is created (and before returning), call:

```rust
write_initial_properties(&data_dir, port)?;
```

The function already exists in `server_properties.rs`. It writes `eula=true`,
`server-port={port}`, and `online-mode=false`.

---

## B5 — Fix list_macros Name Collision

**File:** `presentation/handlers/macros.rs`

**Problem:** The handler function is named `list_macros`. This shadows the imported
`macro_service::list_macros`. When the handler body calls the service function,
Rust resolves the handler name instead. Compile error.

**Fix:** Rename the handler to `list_macros_handler`. This is already the naming
convention used by the other handlers in the same file (`spawn_macro_handler`,
`kill_macro_handler`). Also update the router in `router.rs` to reference the new name.

---

## B6 — Split instance_service.rs Over 200 Lines

**File:** `application/instance_service.rs`

**Problem:** The file is 223 lines, violating the 200-line hard cap.

**Fix:** After B3 (repo wiring) is done, split into two files by responsibility:

- `application/instance_service.rs` — create, delete, get, list (instance lifecycle)
- `application/instance_status_service.rs` — start, stop, kill, restart, status polling

Don't just cut the file at line 200. Find the natural boundary between "instance
data management" and "process control" and split there. All logic stays the same;
only the file boundary changes.

**After splitting:** Update `mod` declarations and all handler imports accordingly.
