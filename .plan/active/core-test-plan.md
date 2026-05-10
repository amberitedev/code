# Amberite Core — Test Plan & Issue Registry

> Written: 2026-05-09  
> Status: PLAN — not yet implemented  
> Scope: `apps/core/` only

---

## Part 1 — Known Bugs & Issues

Every confirmed defect found in source code. Grouped by severity.

---

### CRITICAL — Security

**SEC-01: Pairing code brute-forceable**
- File: `src/application/state.rs:108`, `src/presentation/handlers/setup.rs`
- The 6-digit code (100,000 possibilities) has no rate limiting, no lockout, no backoff.
- Any attacker on the local network can brute-force in seconds.
- Fix: Add an `AtomicU8` wrong-attempt counter to `AppState`. After 5 failures, return `429` permanently until restart.

**SEC-02: WebSocket tickets never garbage-collected**
- File: `src/application/state.rs:41` — `ws_tickets: DashMap<String, WsTicket>`
- Expired tickets accumulate forever. Consumed tickets are removed, but unconsumed expired ones are not.
- Fix: Spawn a cleanup task in `main.rs` that drains expired entries every 5 minutes.

**SEC-03: Path traversal guard in log service — untested and unverified**
- File: `src/application/log_service.rs` (guard via `filename.contains("..")`)
- The guard exists but has no test. A crafted filename like `....//etc/passwd` or URL-encoded variants may bypass a naive `..` check.
- Fix: Use `sanitize-filename` crate (already a dep) + reject any path that escapes the instance data dir after canonicalization.

**SEC-04: `delete_instance` silently succeeds on invalid UUID**
- File: `src/presentation/handlers/instances.rs:112`
- If the `:id` param is not a valid UUID, the `if let Ok(iid)` block skips the running-check entirely. The `DELETE FROM instances WHERE id = ?` then affects 0 rows. Response is `{"ok": true}`.
- Fix: Parse UUID at the top, return `400` immediately if invalid.

---

### HIGH — Architecture (Layer Violations)

Every place where a handler or service reaches directly into `state.pool` instead of going through a port.

**ARCH-01: `instances.rs` handlers bypass `InstanceStore` entirely**
- File: `src/presentation/handlers/instances.rs:35–74`
- `list_instances` and `get_instance` both use raw `sqlx::query_as` directly in the handler. The `InstanceStore` port has `.list()` and `.get()` methods that should be called instead.
- The column projection in the handler will silently drift from the port's version.

**ARCH-02: `delete_instance` handler skips filesystem cleanup**
- File: `src/presentation/handlers/instances.rs:119–123`
- `DELETE FROM instances WHERE id = ?` removes the DB row but leaves `data/instances/<id>/` on disk forever.
- Fix: Create a `delete_instance` service function that calls `instance_store.delete()` then `tokio::fs::remove_dir_all(data_dir)`.

**ARCH-03: `instance_actor` writes SQL directly instead of using the port**
- File: `src/infrastructure/process/instance_actor.rs:118–128` — `set_status` function
- The actor is deep in infrastructure but calls `sqlx::query("UPDATE instances SET status = ...")` directly instead of `state.instance_store.update_status()`. The correct method already exists on the port.
- This makes the actor untestable with `MockSpawner` because status updates silently require a real DB.

**ARCH-04: `instance_service.rs` bypasses port for `update_port`**
- File: `src/application/instance_service.rs:113` (approximate)
- Raw `sqlx::query("UPDATE instances SET port = ?")` in a service. `InstanceStore` has no `update_port` method so the developer went around it.
- Fix: Add `update_port(&self, id: &InstanceId, port: u16)` to `InstanceStore` trait and `InstanceRepo` impl.

**ARCH-05: `mod_service.rs` — entire file is raw SQL with no port**
- File: `src/application/mod_service.rs` (entire file, 183 lines)
- No `ModStore` port exists. Every operation (`list_mods`, `add_mod`, `upload_mod`, `delete_mod`, `toggle_mod`, `update_mod`, `update_all_mods`) reaches directly into `state.pool`.
- This is the largest missing abstraction in the system. None of mod logic can be tested without a real SQLite.
- Fix: Create `ports/mod_store.rs` with a `ModStore` trait. Implement in `infrastructure/db/mod_repo.rs`. Thread `Arc<dyn ModStore>` through `AppState`.

**ARCH-06: `log_service.rs` queries `data_dir` via raw SQL**
- File: `src/application/log_service.rs:23` (approximate)
- `sqlx::query_as("SELECT data_dir FROM instances WHERE id = ?")` inside the service. Should call `instance_store.get(id)` and read `.data_dir` from the returned domain object.

**ARCH-07: `setup.rs` handler writes `core_config` via raw SQL**
- File: `src/presentation/handlers/setup.rs:36–45`
- No `ConfigStore` port. A handler writes directly to `core_config` table. Config reads also scattered across `state.rs` and `setup.rs`.
- Fix: Create `ports/config_store.rs` with `get_config() -> Option<CoreConfig>` and `set_config(config: CoreConfig)`. Implement in `infrastructure/db/config_repo.rs`.

---

### HIGH — Panics in Production Code

**PANIC-01: `logs.rs` — `.unwrap()` on `Response::builder().body()`**
- File: `src/presentation/handlers/logs.rs:52` and `:82`
- `Response::builder().body(Body::from(data)).unwrap()` — if the builder has an invalid state, this panics in a live request handler, crashing that request thread.
- Fix: `.map_err(|e| ApiError::Internal(e.to_string()))?`

**PANIC-02: `modpack.rs` — same pattern**
- File: `src/presentation/handlers/modpack.rs:102` (approximate)
- Same `.unwrap()` on response builder. Same fix.

**PANIC-03: `restore_instances` silently swallows DB failures**
- File: `src/application/instance_service.rs` — `restore_instances()`
- `list_by_status(...).unwrap_or_default()` — if the DB is unavailable at startup, instances are silently not restored with no log and no error. The server starts cleanly and appears healthy.
- Fix: `match` on the result; `tracing::error!` on `Err`, then `return` or `vec![]` with an explicit log.

**PANIC-04: `state.rs` masks DB connectivity error at startup**
- File: `src/application/state.rs:65`
- `fetch_one(...).unwrap_or(0)` — if the DB connection fails here, the Core starts in "not paired" state and generates a fresh pairing code even though the DB has existing config.
- Fix: Propagate the error with `?` — this is in `AppState::new` which already returns `Result`.

---

### MEDIUM — Behavioral Bugs

**BEH-01: `send_command` drops messages silently under load**
- File: `src/infrastructure/process/instance_actor.rs:73` (cmd dispatch in handler)
- `instance_control.rs` uses `try_send` (non-blocking, drops message if channel full) for `SendCommand`, but `stop_instance` uses `.send().await` (async, backpressures).
- Under burst (rapid commands), `try_send` silently discards commands instead of returning a `503` to the caller.
- Fix: Both paths should use `.send().await.map_err(|_| StatusError::ActorDead)`.

**BEH-02: `restart_instance` busy-polls with 500ms sleep**
- File: `src/application/instance_status_service.rs` (restart function)
- Loop with `sleep(Duration::from_millis(500))` waiting for the instance to leave the `instances` DashMap. Up to 500ms wasted latency. Wrong error variant on the 30s timeout.
- Fix: Use a `tokio::sync::Notify` signaled by the actor on exit, or subscribe to the `StatusChanged` broadcast event and wait for `Offline`.

**BEH-03: `list_macros` returns all running PIDs globally**
- File: `src/presentation/handlers/macros.rs` (list handler)
- Not filtered by instance ID — returns every running macro for every instance.
- Fix: Filter by `instance_id` in `macro_service::list_macros`.

**BEH-04: `create_instance` returns `200 OK` instead of `201 Created`**
- File: `src/presentation/handlers/instances.rs:103`
- REST convention: resource creation should return `201 Created`.
- Fix: `(StatusCode::CREATED, Json(json!({ "id": id }))).into_response()`

---

### LOW — Code Quality

**QUAL-01: `mod_service.rs` field layout breaks formatting**
- File: `src/application/mod_service.rs:15–19`
- `ModInfo` struct has multiple fields on one line. `cargo fmt` would expand this. Also the file is 183 lines — at the 200-line hard limit.

**QUAL-02: Pairing code displayed via `println!` instead of `tracing`**
- File: `src/application/state.rs:71–74`
- Uses raw `println!` while everything else uses `tracing`. Bypasses log capture in tests. Use `tracing::warn!` with a high-visibility prefix, or a dedicated stdout write that's clearly intentional.

**QUAL-03: `clap` dependency is declared but no CLI exists**
- File: `Cargo.toml:110`
- `clap = { version = "4.5", features = ["derive"] }` is declared but `main.rs` does not parse any arguments. The binary accepts no flags.
- Fix: Implement the CLI (see Part 4).

**QUAL-04: No `#[tracing::instrument]` on any service function**
- All service files (`mod_service`, `modpack_service`, `export_service`, etc.) produce zero spans. Production debugging and correlation are impossible without explicit instrumentation.

**QUAL-05: Path params as `Path<String>` instead of `Path<Uuid>`**
- Most handlers: `Path(id): Path<String>` then manually parse later (or not at all).
- A malformed UUID returns a `500` in some paths or silently no-ops in others.
- Fix: `Path(id): Path<Uuid>` at the boundary, then `let id = InstanceId(id);` immediately after.

**QUAL-06: `manifest_to_value` manually serializes a struct**
- File: `src/presentation/handlers/modpack.rs`
- Hand-crafted JSON map instead of deriving `Serialize` on `ModpackManifest`. Will silently miss fields when the struct changes.

---

## Part 2 — Test Infrastructure

### New dev-dependencies needed

Add to `Cargo.toml [dev-dependencies]`:

```toml
[dev-dependencies]
tokio          = { version = "1.47", features = ["full"] }
axum-test      = "15"         # real HTTP against Router without binding a port
sqlx           = { version = "=0.9.0-alpha.1", features = ["runtime-tokio", "tls-rustls", "sqlite", "uuid", "chrono"] }
tempfile       = "3.15"       # temp dirs for per-test data isolation
reqwest        = { version = "0.12", features = ["json"] }
wiremock       = "0.6"        # mock Modrinth API + JWKS endpoint
jsonwebtoken   = "9.3"        # sign test JWTs with known RS256 keypair
rstest         = "0.18"       # parameterized tests (path traversal, bad inputs)
serde_json     = "=1.0.133"
```

### Test harness: `tests/common/`

Everything integration tests need lives here. No test file should duplicate setup logic.

**`tests/common/mod.rs` — `TestApp`**
```
pub struct TestApp {
    pub client: reqwest::Client,    // pre-configured with base_url
    pub base_url: String,           // http://127.0.0.1:<port>
    pub state: Arc<AppState>,       // for direct state inspection
    _data_dir: tempfile::TempDir,   // dropped at end = auto cleanup
}

impl TestApp {
    pub async fn spawn() -> Self
    pub async fn spawn_dev() -> Self   // dev_mode=true, no JWT needed
    pub async fn spawn_paired() -> Self  // already has core_config row
}
```

`TestApp::spawn()` does:
1. `tempfile::tempdir()` → data dir
2. `SqlitePool::connect(":memory:")` — ephemeral DB, isolated per test
3. `sqlx::migrate!("./migrations").run(&pool).await`
4. `AppState::new(config, pool)` with `dev_mode = false`
5. `create_router(state)` → bind on `127.0.0.1:0` → record port
6. `tokio::spawn(axum::serve(...))`
7. Return `TestApp` with configured `reqwest::Client`

**`tests/common/auth.rs` — JWT helpers**

Contains a test RS256 keypair baked as constants (not secrets — test-only). Provides:
```
pub fn sign_test_jwt(sub: &str, role: &str, exp_offset_secs: i64) -> String
pub fn expired_jwt() -> String      // exp in the past
pub fn tampered_jwt() -> String     // valid structure, invalid signature
pub fn jwt_wrong_alg() -> String    // alg: "none" or HS256
pub fn auth_header(token: &str) -> (&str, String)  // ("Authorization", "Bearer <token>")
```

The test JWKS mock server serves the public key from the test keypair so `JwksCache::validate()` works against it.

**`tests/common/fixtures.rs` — data factories**
```
pub async fn create_test_instance(app: &TestApp) -> String  // returns instance id
pub fn default_create_body() -> serde_json::Value
pub fn default_pairing_payload(code: &str) -> serde_json::Value
```

---

## Part 3 — Test Plan

### Tier 1: Unit Tests (colocated, zero I/O)

Live inside `#[cfg(test)] mod tests { }` at the bottom of each source file.

---

**`src/domain/instance.rs`**

```
test: instance_status_roundtrip
  - InstanceStatus::Running.to_string() == "Running"
  - "Running".parse::<InstanceStatus>() == Ok(Running)
  - "garbage".parse::<InstanceStatus>() is Err

test: mod_loader_roundtrip  
  - All variants: Vanilla, Fabric, Forge, NeoForge, Quilt, Paper
  - to_string() then parse() returns same variant

test: memory_settings_default
  - MemorySettings::default().min_mb == 512 (or whatever the constant is)
  - min_mb <= max_mb (no upside-down range)

test: instance_id_display_and_parse
  - InstanceId(Uuid::new_v4()).to_string() is a valid UUID string
  - Parsed back produces same UUID
```

---

**`src/presentation/extractors.rs`**

```
test: bearer_token_valid
  - headers with "Authorization: Bearer abc123" → Some("abc123")

test: bearer_token_missing
  - no Authorization header → None

test: bearer_token_wrong_scheme
  - "Authorization: Basic abc123" → None

test: bearer_token_just_bearer
  - "Authorization: Bearer " (empty token) → Some("") — caller validates emptiness

test: bearer_token_case
  - "authorization: Bearer abc" (lowercase) → Some("abc")  (header names are case-insensitive)
```

---

**`src/application/stats_service.rs`**

```
test: parse_player_count_normal
  - "There are 3 of a max of 20 players online" → Some(3)

test: parse_player_count_zero
  - "There are 0 of a max of 20 players online" → Some(0)

test: parse_player_count_malformed
  - "There are players" → None  (no panic)

test: parse_player_count_empty
  - "" → None
```

---

**`src/application/log_service.rs`**

```
test: path_traversal_dotdot → Err
test: path_traversal_encoded_dotdot  ("%2e%2e") → Err
test: path_traversal_double_slash → Err
test: valid_log_filename → Ok
test: valid_gzip_filename → Ok
test: path_stays_within_dir_after_canonicalize
  - Construct a symlink pointing outside the instance dir → Err
```

---

**`src/application/state.rs`**

```
test: generate_pairing_code_is_six_digits
  - Code is always 6 characters, all numeric, range 100000–999999
  - Run 1000 times, assert all pass (probabilistic coverage)
```

---

### Tier 2: Integration Tests (full HTTP, in-memory DB)

All live in `tests/`. Each file uses `TestApp::spawn()`.

---

**`tests/health.rs`**

```
test: get_health_returns_200
  GET /health → 200, body contains "ok": true

test: get_version_returns_semver
  GET /version → 200, body has "version" string matching semver

test: get_java_returns_list
  GET /java → 200, body has "installations" array (may be empty in CI)
```

---

**`tests/setup.rs`**

```
test: setup_status_unpaired
  GET /setup/status → 200, {"paired": false}

test: setup_wrong_code_returns_403
  POST /setup {"code": "000000", ...} → 403

test: setup_wrong_code_five_times_then_lockout   [SEC-01 regression]
  POST /setup wrong code 6 times → 6th returns 429

test: setup_correct_code_pairs_core
  Read pairing code from AppState
  POST /setup {"code": "<code>", "supabase_url": "...", "anon_key": "..."} → 200
  GET /setup/status → {"paired": true}

test: setup_idempotent_after_paired
  Already paired → POST /setup → 409 or 400

test: setup_status_after_restart  (spawn_paired variant)
  TestApp::spawn_paired() → GET /setup/status → {"paired": true}
```

---

**`tests/auth.rs`**

```
test: no_token_returns_401
  GET /instances (no Authorization header) → 401

test: expired_token_returns_401
  Authorization: Bearer <expired_jwt()> → 401

test: tampered_token_returns_401
  Authorization: Bearer <tampered_jwt()> → 401

test: wrong_alg_returns_401  [SEC JWT]
  Authorization: Bearer <jwt_wrong_alg()> → 401

test: valid_token_passes
  Authorization: Bearer <sign_test_jwt("user","authenticated",3600)>
  GET /instances → 200

test: dev_mode_no_token_passes
  TestApp::spawn_dev() → GET /instances (no token) → 200

test: ws_ticket_flow
  POST /ws-token (auth) → {"ticket": "<uuid>"}
  GET /instances/:id/console?ticket=<uuid> → 101 Upgrade

test: ws_ticket_replay  [SEC-02 regression]
  Issue ticket → use it once → try again with same ticket → 401

test: ws_ticket_expired  [SEC-02 regression]
  Issue ticket, wait past TTL → attempt upgrade → 401
```

---

**`tests/instances.rs`**

```
test: list_instances_empty
  dev mode → GET /instances → {"instances": []}

test: create_instance_returns_id
  POST /instances {valid body} → 200 with {"id": "<uuid>"}
  (currently returns 200, should be 201 — test documents current behavior and tracks BEH-04)

test: create_instance_missing_field_returns_422
  POST /instances {name missing} → 422 Unprocessable Entity

test: create_instance_bad_loader_returns_422
  POST /instances {"loader": "notaloader"} → 422

test: get_instance_returns_record
  Create → GET /instances/:id → 200, fields match

test: get_instance_not_found
  GET /instances/00000000-0000-0000-0000-000000000000 → 404

test: get_instance_invalid_uuid  [QUAL-05, SEC-04 regression]
  GET /instances/not-a-uuid → 400 (not 500, not 200)

test: delete_instance_removes_record
  Create → DELETE /instances/:id → 200
  GET /instances/:id → 404

test: delete_instance_cleans_filesystem  [ARCH-02 regression]
  Create → check data dir exists → DELETE → check data dir gone

test: delete_instance_invalid_uuid  [SEC-04 regression]
  DELETE /instances/not-a-uuid → 400

test: delete_instance_while_running_returns_conflict
  Create + start → DELETE → 409

test: list_instances_shows_created
  Create 3 instances → GET /instances → list has 3 items
```

---

**`tests/instance_lifecycle.rs`**

Uses `MockSpawner` to avoid real Java processes.

```
test: start_instance_transitions_to_starting
  Create → POST /instances/:id/start → 200
  GET /instances/:id → status == "Starting" or "Running"

test: stop_running_instance
  Create → start (mock) → stop → status == "Offline"

test: kill_running_instance
  Create → start (mock) → kill → status == "Offline" or "Crashed"

test: restart_transitions_correctly
  start → restart → end status is "Running"

test: start_already_running_returns_conflict
  start → start again → 409

test: stop_offline_instance_returns_conflict
  Create → stop without starting → 409

test: send_command_to_running
  start → POST /instances/:id/command {"command": "say hello"} → 200

test: send_command_to_offline_returns_conflict
  Create → send command → 409

test: actor_sets_running_on_done_signal  [ARCH-03 verification]
  MockSpawner emits "Done (..." line → actor updates status to Running
  Verify via GET /instances/:id → status == "Running"
  (This test will FAIL until ARCH-03 is fixed, proving the bug exists)
```

---

**`tests/mods.rs`**

Uses `wiremock` to mock Modrinth API.

```
test: list_mods_empty
  Create instance → GET /instances/:id/mods → {"mods": []}

test: list_mods_includes_untracked_jars
  Create instance → manually drop a .jar into data_dir/mods/ → list → appears as untracked mod

test: add_mod_from_modrinth
  Mock Modrinth: version endpoint + project endpoint + download
  POST /instances/:id/mods {"version_id": "abc"} → 200, returns ModInfo
  GET /instances/:id/mods → list contains the mod

test: add_mod_client_only_returns_400
  Mock Modrinth: project has server_side = "unsupported"
  POST /instances/:id/mods → 400

test: upload_mod_jar
  POST /instances/:id/mods/upload (multipart, fake .jar bytes) → 200
  GET /instances/:id/mods → contains filename

test: delete_mod
  Add mod → DELETE /instances/:id/mods/:filename → 200
  GET /instances/:id/mods → not in list
  File should be gone from disk

test: delete_mod_not_found
  DELETE /instances/:id/mods/nonexistent.jar → 404

test: toggle_mod_disable
  Add mod → PATCH /instances/:id/mods/:filename {"enabled": false} → 200
  File renamed to .jar.disabled on disk

test: toggle_mod_enable
  Disable mod → PATCH enable → 200, file renamed back to .jar

test: update_mod_already_latest
  Mock Modrinth: latest version == installed version
  PUT /instances/:id/mods/:filename/update → 200, {"updated": false}

test: update_mod_new_version
  Mock Modrinth: newer version available
  PUT → 200, {"updated": true}
  Old file gone, new file present on disk
```

---

**`tests/modpack.rs`**

```
test: get_modpack_not_installed
  GET /instances/:id/modpack → 404

test: install_modpack (mocked download)
  Mock Modrinth modpack endpoint
  POST /instances/:id/modpack {"version_id": "..."} → 200 or 202
  GET /instances/:id/modpack → returns manifest

test: remove_modpack
  Install → DELETE /instances/:id/modpack → 200
  GET /instances/:id/modpack → 404

test: export_modpack
  Install mods → GET /instances/:id/modpack/export → 200, Content-Type: application/zip
  Zip is valid and contains modrinth.index.json
```

---

**`tests/logs.rs`**

```
test: list_logs_empty_dir
  GET /instances/:id/logs → {"logs": []}

test: list_logs_returns_filenames
  Write fake log files to data_dir → list → filenames appear

test: read_log_returns_content
  Write "hello log" to data_dir/logs/latest.log → GET /instances/:id/logs/latest.log → body == "hello log"

test: read_log_gzipped
  Write gzip bytes → GET → Content-Encoding: gzip header present

test: list_crash_reports
  Write fake crash report → GET /instances/:id/crash-reports → appears in list

test: read_crash_report
  Write crash text → GET /instances/:id/crash-reports/:filename → body matches
```

---

**`tests/properties.rs`**

```
test: get_properties_returns_map
  Create instance with server.properties present → GET → map of key/value pairs

test: patch_properties_updates_file
  GET → patch one key → GET again → value updated

test: patch_properties_unknown_key
  PATCH with an unknown key → behavior TBD (400 or just skip?)
```

---

**`tests/stats.rs`**

```
test: get_stats_offline_instance
  GET /instances/:id/stats → 200, cpu_percent and ram_bytes present (may be 0)

test: get_stats_not_found
  GET /instances/fake-id/stats → 404
```

---

**`tests/macros.rs`**

```
test: list_macros_empty
  GET /instances/:id/macros → {"macros": []}

test: spawn_macro_not_found_script
  POST /instances/:id/macros {"name": "nonexistent"} → 404

test: kill_macro_not_found
  DELETE /instances/:id/macros/99999 → 404
```

---

**`tests/console.rs`**

```
test: issue_ws_token_requires_auth
  POST /ws-token (no token) → 401

test: issue_ws_token_returns_uuid
  POST /ws-token (valid JWT) → 200, {"ticket": "<uuid>"}

test: ws_console_requires_ticket
  GET /instances/:id/console (no ticket param) → 401 or 400

test: ws_console_invalid_ticket
  GET /instances/:id/console?ticket=fake → 401

test: sse_progress_streams_events
  POST /instances (async creation) → GET /instances/:id/progress → SSE stream receives at least one event
```

---

### Tier 3: Security Tests

All in `tests/security/`.

---

**`tests/security/auth.rs`**

```
test: brute_force_pairing_lockout  [SEC-01]
  POST /setup with wrong code × 6 → 6th response is 429

test: jwt_alg_none_rejected  [SEC JWT]
  Craft JWT with alg:none → POST any authenticated endpoint → 401

test: jwt_hs256_rejected  [SEC JWT]
  Sign with HMAC instead of RSA → 401

test: jwt_rs256_wrong_kid  [SEC JWT]
  Valid JWT but kid not in JWKS → 401

test: jwt_expired_rejected
  exp = now - 1 → 401

test: jwt_future_iat_rejected
  iat = now + 9999 → 401 (if iat validation exists, else document missing)
```

---

**`tests/security/path_traversal.rs`**

Uses rstest for parameterized bad filenames.

```
#[rstest]
#[case("../../../etc/passwd")]
#[case("..%2f..%2fetc%2fpasswd")]
#[case("....//etc/passwd")]
#[case("logs/../../../secret")]
#[case("\x00null_byte")]
fn path_traversal_log_read_rejected(filename: &str)
  GET /instances/:id/logs/<filename> → 400 or 404, never 200

#[rstest]
fn path_traversal_crash_report_rejected(filename: &str)
  GET /instances/:id/crash-reports/<filename> → 400 or 404, never 200
```

---

**`tests/security/large_payloads.rs`**

```
test: create_instance_10mb_body_rejected
  POST /instances with 10MB JSON body → 413 Payload Too Large (need to add body size limit to router)

test: mod_upload_huge_file
  POST /instances/:id/mods/upload with 500MB multipart → should reject or stream cleanly, not OOM
```

---

**`tests/security/invalid_input.rs`**

```
test: invalid_uuid_in_all_routes
  For every route with :id param, send "not-a-uuid" → 400, not 500

test: sql_injection_in_instance_name
  POST /instances with name = "'; DROP TABLE instances; --" → creates fine, name stored as literal string

test: sql_injection_in_filename
  DELETE /instances/:id/mods/"'; DROP TABLE mods; --" → 400 (sanitized filename check)

test: empty_string_name_rejected
  POST /instances {"name": ""} → 422

test: name_too_long_rejected
  POST /instances {"name": "<300 chars>"} → 422 (requires validation to be added)
```

---

## Part 4 — CLI Plan (activate the dead `clap` dep)

`main.rs` should parse a `Cli` struct before doing anything else.

```
amberite-core                  # default: run the server (same as `run`)
amberite-core run              # start server (default)
amberite-core check            # validate config + DB connectivity, exit 0 or 1
amberite-core migrate          # run migrations only, then exit
amberite-core version          # print version + build date + git sha, exit 0
amberite-core reset-pairing    # clear core_config, generate fresh pairing code, exit
```

`check` is the most important subcommand. It is used by Docker healthchecks and orchestration scripts to validate the binary can reach its DB and is configured before starting the server. It must exit `0` on success and `1` on failure.

---

## Part 5 — Docker Plan

### `apps/core/Dockerfile`

Multi-stage build:

```
# Stage 1: builder
FROM rust:1.85-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY patches/ ./patches/
# Cache deps layer
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release && rm -rf src
COPY src/ ./src/
COPY migrations/ ./migrations/
RUN cargo build --release

# Stage 2: runtime
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/amberite-core ./
COPY --from=builder /build/migrations/ ./migrations/

ENV AMBERITE_DATA_DIR=/data
ENV AMBERITE_PORT=16662
ENV RUST_LOG=amberite_core=info

VOLUME /data
EXPOSE 16662

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:16662/health || exit 1

ENTRYPOINT ["./amberite-core"]
CMD ["run"]
```

### `apps/core/docker-compose.yml`

```yaml
services:
  core:
    build: .
    ports:
      - "16662:16662"
    volumes:
      - ./data:/data
    environment:
      - AMBERITE_DATA_DIR=/data
      - AMBERITE_PORT=16662
      - DEV_MODE=true           # disable JWT for local dev
      - RUST_LOG=amberite_core=debug
    restart: unless-stopped
```

---

## Part 6 — AGENTS.md Regeneration Plan

After tests are implemented, delete every existing `AGENTS.md` and regenerate using this target folder structure. Every subfolder gets its own `AGENTS.md`. The file describes ONLY what is in that folder — no cross-layer references.

### Target folder structure

```
apps/core/
├── AGENTS.md                          ← top-level: what is core, how to run, how to test
├── src/
│   ├── domain/
│   │   └── AGENTS.md                 ← pure types: InstanceId, InstanceStatus, ModLoader, Event, Modpack
│   ├── ports/
│   │   └── AGENTS.md                 ← trait interfaces: InstanceStore, ModpackStore, ModStore (new), ConfigStore (new), ProcessSpawner
│   ├── application/
│   │   └── AGENTS.md                 ← services: what each service does, error types, AppState fields
│   ├── infrastructure/
│   │   ├── AGENTS.md                 ← overview: what each subdir provides
│   │   ├── auth/
│   │   │   └── AGENTS.md             ← JwksCache, Claims, RS256 validation, JWKS URL format
│   │   ├── db/
│   │   │   └── AGENTS.md             ← InstanceRepo, ModpackRepo, ModRepo (new), ConfigRepo (new), migration notes
│   │   ├── process/
│   │   │   └── AGENTS.md             ← PtySpawner, MockSpawner, InstanceHandle, ActorCmd, spawn_actor
│   │   ├── minecraft/
│   │   │   └── AGENTS.md             ← server_jar, installer, flavours, java detection, mrpack, modrinth_api
│   │   └── macro_engine/
│   │       ├── AGENTS.md             ← Deno executor, loader, LocalSet requirement
│   │       └── ops/
│   │           └── AGENTS.md         ← Deno op implementations: instance_control, events, prelude
│   └── presentation/
│       ├── AGENTS.md                 ← router map (method + path + auth), ApiError, AuthUser
│       └── handlers/
│           └── AGENTS.md             ← per-handler file list, signature convention, 201 rule, ≤40 lines rule
└── tests/
    ├── AGENTS.md                     ← how to run tests, TestApp, auth helpers, fixtures, wiremock
    ├── common/
    │   └── AGENTS.md                 ← TestApp::spawn variants, sign_test_jwt, create_test_instance
    └── security/
        └── AGENTS.md                 ← adversarial tests, SEC-* issue refs, rstest usage
```

### What each AGENTS.md must contain

Every file must have:

1. **One-line description** of what this folder is responsible for
2. **File table** — filename | purpose (every .rs file in the folder)
3. **Key types / traits / functions** — the public API of the folder with signatures
4. **Rules** — what is and is not allowed in this folder (e.g., "no axum imports here")
5. **Known issues** — list of open ARCH-*, SEC-*, BEH-*, PANIC-* items that live in this folder
6. **Test coverage** — what is tested, what is not yet tested

---

## Part 7 — Implementation Order

1. Add dev-dependencies to `Cargo.toml`
2. Write `tests/common/` harness (TestApp, auth helpers, fixtures)
3. Write Tier 1 unit tests (domain, extractors, stats, log path traversal)
4. Write `tests/health.rs`, `tests/auth.rs`, `tests/setup.rs`
5. Write `tests/instances.rs` — this will expose ARCH-01, ARCH-02, SEC-04 as failing tests
6. Write `tests/instance_lifecycle.rs` — this will expose ARCH-03, BEH-01, BEH-02 as failing tests
7. Write `tests/security/` — document which tests fail due to open bugs
8. Write remaining integration tests (mods, modpack, logs, properties, stats, console)
9. Fix all bugs exposed by failing tests (ARCH-*, SEC-*, PANIC-*, BEH-*)
10. Add CLI (`clap` subcommands in `main.rs`)
11. Write `Dockerfile` + `docker-compose.yml`
12. Delete all existing `AGENTS.md` files
13. Regenerate `AGENTS.md` for every folder using the structure in Part 6
