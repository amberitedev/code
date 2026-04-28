# infrastructure/macro_engine/

Deno Core JS/TS runtime for user-authored macros.

## Files

| File | Purpose |
|------|---------|
| `executor.rs` | `MacroExecutor` — spawn/kill/list macro processes |
| `loader.rs` | `TypescriptModuleLoader` — transpiles TS → JS on load |
| `ops/prelude.rs` | `amberite_prelude` extension — initialises `op_state` |
| `ops/events.rs` | `amberite_events` extension — `Amberite.on(event, cb)` API |
| `ops/instance_control.rs` | `amberite_instance_control` extension — `Amberite.sendCommand(id, cmd)` |
| `ops/mod.rs` | Re-exports |

## How a macro runs

1. `MacroExecutor::spawn_macro()` allocates a `MacroPid`, then spawns a **std::thread** (Deno requires single-threaded `LocalSet`).
2. Inside the thread, a new `JsRuntime` is created with the three extensions.
3. `op_state` is populated with `Arc<AppState>`, `InstanceId`, `Arc<Mutex<broadcast::Receiver<Event>>>`, and the `MacroPid`.
4. The `.ts` or `.js` file is loaded as an ES module; the event loop runs to completion.
5. The `v8::IsolateHandle` is registered in `MacroExecutor.processes` so `kill_macro()` can call `terminate_execution()`.

## Macro file location

```
data_dir/instances/<instance_id>/macros/<name>.ts   (preferred)
data_dir/instances/<instance_id>/macros/<name>.js   (fallback)
```

## Rules

- Each macro runs in its own V8 isolate — full isolation.
- Do not share `JsRuntime` across instances or macros.
- `op_state_fn` on `Extension` must set all required state before the runtime starts.
- Deno Core version: `0.354` — `RuntimeOptions` has no `op_state_fn`; attach it directly to the extension struct.
