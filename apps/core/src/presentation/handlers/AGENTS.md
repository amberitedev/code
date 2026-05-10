# presentation/handlers/

One handler file per feature area. Handlers extract state, call an `application/` service, and return `Result<impl IntoResponse, ApiError>`. No business logic here.

## Files

| File | Handlers |
|------|---------|
| `diagnostics.rs` | `health` (GET /health), `version` (GET /version), `java_installations` (GET /java) |
| `setup.rs` | `complete_setup` (POST /setup), `setup_status` (GET /setup/status) |
| `instances.rs` | `list_instances`, `create_instance`, `get_instance`, `delete_instance` |
| `instance_control.rs` | `start`, `stop`, `kill`, `restart`, `send_command_handler` |
| `console.rs` | `ws_console` (WS), `sse_progress` (SSE), `issue_ws_token` |
| `modpack.rs` | `install_modpack`, `get_modpack`, `remove_modpack`, `export_modpack_handler` |
| `macros.rs` | `list_macros_handler`, `spawn_macro_handler`, `kill_macro_handler` |
| `mods.rs` | `list_mods_handler`, `add_mod_handler`, `upload_mod_handler`, `delete_mod_handler`, `toggle_mod_handler`, `update_mod_handler`, `update_all_handler` |
| `logs.rs` | `list_logs_handler`, `read_log_handler`, `list_crash_reports_handler`, `read_crash_report_handler` |
| `properties.rs` | `get_properties_handler`, `patch_properties_handler` |
| `stats.rs` | `get_stats_handler` |
| `mod.rs` | Re-exports all handler modules |

## Handler conventions

```rust
// Typical signature
pub async fn my_handler(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<SomeDto>,
) -> Result<Json<Value>, ApiError>
```

- UUID path params parsed explicitly: `id.parse::<InstanceId>().map_err(|_| ApiError::BadRequest(...))`
- JSON body DTOs defined in the same file if ≤5 fields.
- File uploads use `axum::extract::Multipart`.
- SSE uses `axum::response::Sse` + `tokio_stream::wrappers::BroadcastStream`.
- WS uses `axum::extract::WebSocketUpgrade`.

## Key implementation notes

### `setup.rs` — SEC-01

`complete_setup` checks `state.wrong_pairing_attempts` (AtomicU32). Returns 429 after 5 wrong attempts. Counter resets to 0 on success. `MAX_PAIRING_ATTEMPTS = 5`.

### `instances.rs` — BEH-04 + SEC-04

- `create_instance` returns **201 Created** with full `InstanceDetail` JSON (BEH-04 **FIXED**).
- `delete_instance` parses UUID at the top before hitting the DB (SEC-04 **FIXED**).
- `delete_instance` returns 409 if the instance has an active `InstanceHandle` in `AppState.instances`.

### `instances.rs` + `instance_control.rs` — ARCH-01

`list_instances` and `get_instance` use raw `sqlx::query_as` on `AppState.pool` instead of `instance_store`. This is a known ARCH-01 open bug.

### `logs.rs`

Path traversal guard is in `log_service::resolve_log` — not in the handler itself.

## Rules

- No raw SQL in handlers — call `application/` services.
- All handlers must be ≤ 40 lines; extract private helpers if longer.
- DTO structs may live in handler files if small; otherwise extract to a `dto.rs`.
- Do not import `sqlx` directly in handler files.
