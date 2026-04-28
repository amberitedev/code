# Part 4 — Log History & Server Properties

---

## Log History API

The Core already streams live console output via WebSocket. This section adds the ability
to list and read historical log files from past server sessions.

New files to create:
- `application/log_service.rs`
- `presentation/handlers/logs.rs`

### Where Minecraft stores logs

- **Session logs:** `{data_dir}/logs/` — current session is `latest.log`, archived
  sessions are `{date}-{n}.log.gz` (gzip compressed)
- **Crash reports:** `{data_dir}/crash-reports/` — plain text `.txt` files

---

### LOG1 — GET /instances/:id/logs

**What:** Lists all historical log files for an instance.

**Steps:**
1. Check that `{data_dir}/logs/` exists. If the instance has never started, it won't.
2. Scan the directory. Include both `.log` and `.log.gz` files.
3. Return array sorted newest-first by `modified_at`.

**Response entry:**
```json
{ "filename": "2025-01-15-1.log.gz", "size_bytes": 45231, "modified_at": "2025-01-15T22:10:00Z" }
```

---

### LOG2 — GET /instances/:id/logs/:filename

**What:** Returns the content of a specific log file as plain text.

**Steps:**
1. Validate `filename` — reject any value containing `..`, `/`, or `\`. Only accept
   a plain filename with no path separators. Return 400 if invalid.
2. Construct `{data_dir}/logs/{filename}`. Return 404 if it doesn't exist.
3. If `filename` ends in `.gz`, decompress using the `flate2` crate before streaming.
4. Stream the response body — do NOT read the entire file into memory first.
   Large log files on busy servers regularly exceed 50 MB.

**Don't:** Allow path traversal. A request for `../../../etc/passwd` must be rejected.
**Don't:** Load the whole file into a `Vec<u8>` and return it. Stream with
`axum::body::Body::from_stream`.

---

### LOG3 — GET /instances/:id/crash-reports

**What:** Lists all crash report files.

Same logic as LOG1 but scan `{data_dir}/crash-reports/`. Crash reports are plain `.txt`
files. Sort newest-first.

---

### LOG4 — GET /instances/:id/crash-reports/:filename

**What:** Returns the content of a specific crash report.

Same logic as LOG2 but from `{data_dir}/crash-reports/`. Crash reports are never
compressed. Same filename validation rules apply.

---

## Server Properties API

Minecraft's `server.properties` file controls core server behavior. Currently the Core
writes it once at instance creation but never exposes it via HTTP.

New files to create:
- `presentation/handlers/properties.rs`

The read/write logic lives in `infrastructure/minecraft/server_properties.rs` (already
exists). Add the HTTP endpoints on top of it.

---

### PROP1 — GET /instances/:id/properties

**What:** Reads `server.properties` and returns it as a JSON object for the dashboard.

**Steps:**
1. Open `{data_dir}/server.properties`
2. Parse line by line: skip lines starting with `#` and blank lines
3. Split each remaining line on the **first** `=` only (values can contain `=`)
4. Return as flat JSON: `{ "server-port": "25565", "max-players": "20", ... }`

**Don't:** Use a TOML or INI parser. `server.properties` has its own format that won't
parse correctly with those. Do it manually.

---

### PROP2 — PATCH /instances/:id/properties

**What:** Updates one or more fields in `server.properties` without destroying the file.

**Request:**
```json
{ "max-players": "40", "view-distance": "12" }
```

**Steps:**
1. Read the entire file into a list of lines
2. For each line, if its key matches a key in the request body, replace the value
3. For keys in the request body that were NOT found in the file, append them at the end
4. Write the modified lines back to the file, preserving all `#` comment lines exactly

**Special case — server-port:** If the request includes `"server-port"`, also update the
`port` column in the `instances` table. The DB and the file must stay in sync.

**Don't:** Rewrite the file from scratch — it would destroy the Minecraft-generated
comments at the top. Read, modify in-place, write back.

**Include in response:**
```json
{
  "updated": ["max-players", "view-distance"],
  "note": "Most changes take effect after server restart."
}
```

---

## Router Wiring

Add the new routes in `presentation/router.rs`:

```
GET  /instances/:id/logs                    → logs::list_logs_handler
GET  /instances/:id/logs/:filename          → logs::read_log_handler
GET  /instances/:id/crash-reports           → logs::list_crash_reports_handler
GET  /instances/:id/crash-reports/:filename → logs::read_crash_report_handler
GET  /instances/:id/properties              → properties::get_properties_handler
PATCH /instances/:id/properties             → properties::patch_properties_handler
```
