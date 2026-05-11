# src/infrastructure/macro_engine — Embedded Deno V8 runtime

Executes user-authored JavaScript/TypeScript macros in isolated V8 contexts. Each macro has access to Amberite-specific Deno extensions that let it read instance output, send commands, and query status.

## File structure

```
macro_engine/
  mod.rs          — re-exports: executor, loader, ops
  executor.rs     — MacroExecutor: spawns/kills/lists macros; tracks live PIDs
  loader.rs       — TypescriptModuleLoader: loads and transpiles .ts/.js from filesystem
  ops/
    mod.rs              — re-exports: prelude, events, instance_control
    prelude.rs          — amberite_prelude extension: op_get_version
    events.rs           — amberite_events extension: op_next_instance_output, op_next_state_change
    instance_control.rs — amberite_instance_control extension: op_send_command, op_get_status, op_stop_instance
```

## executor.rs — MacroExecutor

`MacroExecutor` holds a `DashMap<MacroPid, v8::IsolateHandle>` and an `AtomicU64` PID counter.

**Threading model**: Each macro runs in a `std::thread::spawn` (NOT a tokio task). This is required because `JsRuntime` contains `Rc<>` and `*mut v8::Isolate` which are not `Send`. Inside the thread, a `tokio::task::LocalSet` is used to run async operations on the current thread, accessed via `rt_handle.block_on(local.run_until(...))`.

`spawn_macro`: allocates a PID, spawns a thread, registers the isolate handle for `kill_macro`, evaluates the module, then removes the PID on exit.

`kill_macro`: calls `v8::IsolateHandle::terminate_execution()` — this terminates the V8 isolate's event loop at the next JS safe point. Returns `false` if the PID was not found (already exited).

**Op state injection**: The prelude extension's `op_state_fn` field is used to inject `Arc<AppState>`, `InstanceId`, `EventRx` (broadcast receiver), and the macro's own PID into the Deno op state, making them accessible to all extensions.

## loader.rs — TypescriptModuleLoader

Loads from the local filesystem only; transpiles `.ts` via `deno_ast` before handing to V8. HTTP imports fail — not supported.

## ops/ — Deno extensions

### amberite_prelude

Registers `op_get_version` — returns the Core version string. Also the extension that injects `AppState`, `InstanceId`, `EventRx`, and `MacroPid` into op state via `op_state_fn`.

### amberite_events

- `op_next_instance_output`: async; awaits the next `Event::InstanceOutput` for the macro's instance. Returns the line string.
- `op_next_state_change`: async; awaits the next `Event::StatusChanged` for the macro's instance. Returns the new status string.

### amberite_instance_control

Synchronous ops backed by `AppState`:

- `op_send_command`: sends a console command to the instance's actor via `ActorCmd::SendCommand`. No-ops if the instance is not in the `instances` map.
- `op_get_status`: reads current status from DB via `instance_store.get()`.
- `op_stop_instance`: sends `ActorCmd::GracefulStop`.

## Gotchas

- **`op_state_fn` API changed in deno_core 0.354+**: `RuntimeOptions` no longer has `op_state_fn`. The workaround is to set `prelude_ext.op_state_fn` directly on the extension struct before passing it to `RuntimeOptions.extensions`. Do not look for `op_state_fn` in `RuntimeOptions`.
- **Kill does not interrupt blocking I/O**: `terminate_execution()` interrupts the V8 event loop at the next JS safe point, but if a macro is blocked inside a native op awaiting a Tokio future (e.g., `op_next_instance_output`), termination may be delayed until the future resolves or the broadcast sender drops.
- **No macro sandbox**: Macros run in the same process and have filesystem access via `deno_core`. There is no permission layer enforced — any macro can read/write arbitrary files.
