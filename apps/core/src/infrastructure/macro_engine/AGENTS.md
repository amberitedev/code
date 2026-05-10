# infrastructure/macro_engine/

Deno Core JS/TS runtime for user-authored server automation macros.

## Files

| File | Purpose |
|------|---------|
| `executor.rs` | `MacroExecutor` — spawn / kill / list macro processes |
| `loader.rs` | `TypescriptModuleLoader` — transpiles `.ts` → JS on load |
| `ops/prelude.rs` | `amberite_prelude` extension — populates `op_state` |
| `ops/events.rs` | `amberite_events` extension — `Amberite.on(event, cb)` JS API |
| `ops/instance_control.rs` | `amberite_instance_control` — `Amberite.sendCommand(id, cmd)` |
| `ops/mod.rs` | Re-exports op extensions |
| `mod.rs` | Re-exports |

## `MacroExecutor`

```rust
pub struct MacroExecutor {
    processes: Arc<DashMap<MacroPid, v8::IsolateHandle>>,
    next_pid:  Arc<AtomicU64>,
}
```

### `spawn_macro(instance_id, macro_path, state, event_rx) -> MacroPid`

1. Allocates a `MacroPid` (monotonically incrementing `u64`).
2. Spawns a **`std::thread`** (Deno/V8 requires single-threaded execution).
3. Inside the thread: creates `tokio::task::LocalSet`, runs `block_on`.
4. Builds a `JsRuntime` with three extensions: `amberite_prelude`, `amberite_events`, `amberite_instance_control`.
5. Injects `Arc<AppState>`, `InstanceId`, `Arc<Mutex<broadcast::Receiver<Event>>>`, and `MacroPid` into `op_state` via `prelude_ext.op_state_fn`.
6. Loads `.ts`/`.js` file as an ES module, runs the event loop to completion.
7. Registers the `v8::IsolateHandle` in `processes` so `kill_macro` can terminate it.

### `kill_macro(pid: MacroPid) -> bool`

Removes the isolate handle and calls `terminate_execution()`. Returns `true` if found.

### `list_pids() -> Vec<MacroPid>`

Returns all currently running macro PIDs.

## Macro file location

```
{data_dir}/instances/{instance_id}/macros/{name}.ts   (preferred)
{data_dir}/instances/{instance_id}/macros/{name}.js   (fallback)
```

## Deno Core version note

Version `0.354.0`. `RuntimeOptions` no longer has `op_state_fn` — it must be set directly on the `Extension` struct before passing to `JsRuntime::new`.

## JS API surface (exposed to macro scripts)

```js
// Listen for instance events
Amberite.on("instance_output", ({ instance_id, line }) => { ... });

// Send a command to a server
await Amberite.sendCommand(instanceId, "say Hello");
```

## Rules

- Each macro runs in its own V8 isolate — full isolation, no shared state.
- Do not share `JsRuntime` across instances or macros.
- `op_state_fn` must inject all required state before runtime starts.
- The std::thread is intentional — `LocalSet` cannot run in a Tokio worker thread.
