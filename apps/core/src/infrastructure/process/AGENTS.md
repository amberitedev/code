# infrastructure/process/

Process spawning and the per-instance lifecycle actor.

## Files

| File | Purpose |
|------|---------|
| `pty_spawner.rs` | `PtySpawner` + `PtyHandle` — real PTY via `portable_pty` |
| `instance_actor.rs` | `InstanceHandle`, `ActorCmd`, `spawn_actor` — per-instance task |
| `mock_spawner.rs` | `MockSpawner` — in-memory fake for tests |
| `mod.rs` | Re-exports |

## `PtySpawner`

Implements `ProcessSpawner<Handle = PtyHandle>`.  
Opens a `portable_pty` pair, spawns the child on the slave end, and forwards stdout lines over a `mpsc::channel<String>(512)` via a background `std::thread`.

`PtyHandle` methods:
- `send_stdin(line)` — writes to pty master writer (with `writeln!`)
- `take_stdout_rx()` — consumes the channel receiver (one-shot)
- `is_running()` — calls `child.try_wait()`
- `kill()` — calls `child.kill()`
- `pid()` — `child.process_id()`

## `InstanceHandle` + actor

```rust
pub struct InstanceHandle {
    tx:         mpsc::Sender<ActorCmd>,
    pub status: Arc<RwLock<InstanceStatus>>,
    pub pid:    Option<u32>,
    started_at: Instant,
}
```

`ActorCmd` variants: `Stop`, `Kill`, `SendCommand(String)`.

`spawn_actor(instance_id, launch_args, state) -> InstanceHandle`  
— tokio::spawn loop that:
1. Spawns the Java process via `PtySpawner`.
2. Forwards stdout to `EventBroadcaster` as `Event::InstanceOutput`.
3. Updates `InstanceStatus` and broadcasts `Event::StatusChanged` on exit.
4. Handles `ActorCmd` messages from the channel.

## Rules

- `PtySpawner` is the only real spawner; use `MockSpawner` in tests.
- Never block the Tokio thread in the actor — use `spawn_blocking` for any synchronous I/O.
- `InstanceHandle` lives in `AppState.instances: DashMap<InstanceId, InstanceHandle>`.
