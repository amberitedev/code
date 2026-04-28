# presentation/handlers/

One file per feature area. Each handler extracts state, calls a service function, and returns `Result<impl IntoResponse, ApiError>`.

## Files

| File | Handlers |
|------|---------|
| `diagnostics.rs` | `health`, `version`, `java_installations` |
| `setup.rs` | `complete_setup`, `setup_status` |
| `instances.rs` | `list_instances`, `create_instance`, `get_instance`, `delete_instance` |
| `instance_control.rs` | `start`, `stop`, `kill`, `restart`, `send_command_handler` |
| `console.rs` | `ws_console` (WebSocket), `sse_progress` (SSE), `issue_ws_token` |
| `modpack.rs` | `install_modpack`, `get_modpack`, `remove_modpack`, `export_modpack_handler` |
| `macros.rs` | `list_macros_handler`, `spawn_macro_handler`, `kill_macro_handler` |
| `mods.rs` | `list_mods_handler`, `add_mod_handler`, `upload_mod_handler`, `delete_mod_handler`, `toggle_mod_handler`, `update_mod_handler`, `update_all_handler` |
| `logs.rs` | `list_logs_handler`, `read_log_handler`, `list_crash_reports_handler`, `read_crash_report_handler` |
| `properties.rs` | `get_properties_handler`, `patch_properties_handler` |
| `stats.rs` | `get_stats_handler` |
| `mod.rs` | Re-exports all handler modules |

## Conventions

- Handler signature: `async fn foo(State(state): State<Arc<AppState>>, AuthUser(claims): AuthUser, ...) -> Result<impl IntoResponse, ApiError>`
- Path params extracted with `Path(id): Path<Uuid>` then wrapped: `let id = InstanceId(id);`
- JSON body extracted with `Json(body): Json<SomeDto>` — define the DTO struct in the same file if small.
- File uploads use `Multipart` from `axum::extract::Multipart`.
- SSE uses `axum::response::Sse` with a `tokio_stream::wrappers::BroadcastStream`.
- WS uses `axum::extract::WebSocketUpgrade`.
- `201 Created` for resource creation: `(StatusCode::CREATED, Json(record)).into_response()`.

## Rules

- No business logic in handlers — delegate everything to `application/` services.
- DTO structs may live in handler files if ≤3 fields; otherwise move to a `dto.rs` companion.
- All handlers must be ≤ 40 lines; split helpers into private `fn`s if longer.
