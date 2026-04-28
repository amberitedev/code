# Part 3 — Mod Management

This is the largest new feature. Currently the Core has zero awareness of what mods are
installed. This section adds a full mod library per instance — install, remove, enable,
disable, and update.

The existing `ModrinthClient` in `infrastructure/minecraft/modrinth_api.rs` is fully
implemented. All Modrinth API calls in this section must go through it.

New files to create:
- `application/mod_service.rs`
- `presentation/handlers/mods.rs`
- `migrations/004_mods.sql`

---

## M0 — Database Schema

New migration `004_mods.sql`:

```sql
CREATE TABLE IF NOT EXISTS mods (
    id              TEXT PRIMARY KEY,
    instance_id     TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    filename        TEXT NOT NULL,
    display_name    TEXT,
    modrinth_project_id  TEXT,
    modrinth_version_id  TEXT,
    version_number  TEXT,
    client_side     TEXT,
    server_side     TEXT,
    sha512          TEXT,
    enabled         INTEGER NOT NULL DEFAULT 1,
    installed_at    TEXT NOT NULL,
    UNIQUE(instance_id, filename)
);
```

- `sha512` is required for modpack export (MP2) — store it on install
- `modrinth_project_id` / `modrinth_version_id` are null for private/uploaded mods
- `enabled = 0` means the file on disk is renamed to `filename.disabled`
- `ON DELETE CASCADE` ensures mods are removed when the instance is deleted

---

## Modrinth Environment Filtering

Every mod has `client_side` and `server_side` values: `"required"`, `"optional"`, or
`"unsupported"`.

**Dashboard default:** Hide mods where `server_side = "unsupported"` (client-only mods
that cannot run on a server). Clients pass `?include_client_only=true` to see everything.

**Install guard:** Before downloading any mod, check `server_side`. If `"unsupported"`,
return HTTP 400:
```json
{ "error": "client_only", "message": "This mod is client-only and cannot be installed on a server." }
```

**Don't:** Let client-only mods install silently. They will either crash the server or
be ignored by it — a confusing experience.

---

## M1 — GET /instances/:id/mods

**What:** Returns all mods for an instance, including untracked files.

**Steps:**
1. Query `mods` table WHERE `instance_id = :id`
2. Scan `{data_dir}/mods/` for `.jar` and `.jar.disabled` files not in the DB
3. Merge results — untracked files appear with `"tracked": false`
4. For mods with a `modrinth_version_id`, set `update_available` based on a cached
   version check (or null if not yet checked)

**Response shape per mod:**
```json
{
  "id": "uuid",
  "filename": "fabric-api-0.100.3.jar",
  "display_name": "Fabric API",
  "version_number": "0.100.3+1.21",
  "enabled": true,
  "tracked": true,
  "client_side": "optional",
  "server_side": "required",
  "modrinth_project_id": "P7dR8mSH",
  "update_available": false
}
```

---

## M2 — POST /instances/:id/mods (Add from Modrinth)

**Request:** `{ "version_id": "P7dR8mSH" }`

**Steps:**
1. Call Modrinth `GET /version/{version_id}` → get file list and `project_id`
2. Call Modrinth `GET /project/{project_id}` → get `client_side` and `server_side`
3. If `server_side = "unsupported"` → return HTTP 400
4. Find primary file: look for `"primary": true` in `files[]`; fallback to first `.jar`
5. Download to `{data_dir}/mods/{filename}` via ModrinthClient
6. Insert `mods` row with all metadata + sha512 from the Modrinth file entry

**Don't:** Use `.output()` for the download; stream it. Large mods can be 100MB+.
**Don't:** Skip step 2. The version endpoint alone does not include `client_side`/`server_side`.

---

## M3 — POST /instances/:id/mods/upload (Add Private Mod)

**Request:** Multipart form upload with the `.jar` file.

**Steps:**
1. Write uploaded bytes to `{data_dir}/mods/{original_filename}`
2. Insert `mods` row with `modrinth_project_id = null`, `modrinth_version_id = null`
3. Compute sha512 of the written file and store it

**Don't:** Call the Modrinth API here. It's a private mod — it isn't there.
**Don't:** Do a `server_side` check — user is responsible for private mods.

---

## M4 — DELETE /instances/:id/mods/:filename

**Steps:**
1. Look up the mod in `mods` table
2. Try to delete `{data_dir}/mods/{filename}` — also check for `.disabled` suffix
3. Delete the DB row
4. If the server is running, include `"warning": "Restart required to unload this mod"` in the 200 response

**Return 404** if neither the DB row nor the file exists.

---

## M5 — PATCH /instances/:id/mods/:filename (Enable/Disable)

**Request:** `{ "enabled": false }`

**Steps:**
- Disable: rename `mod.jar` → `mod.jar.disabled` on disk, set `enabled = 0` in DB
- Enable: rename `mod.jar.disabled` → `mod.jar` on disk, set `enabled = 1` in DB

**Don't:** Assume which extension is present. Check for both before deciding which rename
to perform. Return 404 if neither exists.

---

## M6 — PUT /instances/:id/mods/:filename/update (Update One Mod)

**Steps:**
1. Lookup `modrinth_project_id` from DB. If null → HTTP 400 (`"Cannot auto-update a private mod"`)
2. Query Modrinth for the latest version of this project compatible with instance
   `game_version` and `loader`
3. If returned `version_id` equals stored `modrinth_version_id` → return `{ "already_latest": true }`
4. Download new JAR, delete old file, update `mods` row with new version data

**Don't:** Delete the old file until the new download is complete. Download to a temp path,
then swap files atomically.

---

## M7 — POST /instances/:id/mods/update-all

Runs M6 logic for every mod in the instance that has a `modrinth_version_id`.

**Response:**
```json
{
  "updated": ["mod-a.jar", "mod-b.jar"],
  "already_latest": ["mod-c.jar"],
  "failed": [{ "filename": "mod-d.jar", "error": "version not found" }]
}
```

**Don't:** Run updates in parallel. Call Modrinth sequentially to respect rate limits.
A single failed mod should not abort the rest — collect errors and continue.
