# infrastructure/process/

PTY process spawning and the per-instance lifecycle actor.

## Files

| File | Purpose |
|------|---------|
| `pty_spawner.rs` | `PtySpawner` + `PtyHandle` — production PTY via `portable_pty` |
| `mock_spawner.rs` | `MockSpawner` + `MockHandle` — in-memory fake for tests |
| `instance_actor.rs` | `InstanceHandle`, `ActorCmd`, `spawn_actor` — per-instance task |
| `mod.rs` | Re-exports |

## `PtySpawner` + `PtyHandle`

Implements `ProcessSpawner<Handle = PtyHandle>`.

- Opens a `portable_pty` pair, spawns the child on the slave end.
- Forwards stdout lines over `mpsc::channel<String>(512)` via a background `std::thread` (PTY I/O is synchronous).
- `send_stdin(line)` — writes to PTY master with `writeln!`.
- `take_stdout_rx()` — one-shot; returns `None` on second call.
- `is_running()` — `child.try_wait()`.
- `kill()` — `child.kill()`.
- `pid()` — `child.process_id()`.

## `MockSpawner` + `MockHandle` (tests)

Simulates a running process entirely in memory.

- `spawn()` immediately returns a `MockHandle`.
- `send_stdin(line)` stores the line in `Vec<String>` (inspectable after test).
- `take_stdout_rx()` returns a channel the test can write to.
- `is_running()` — controlled by `MockHandle::set_running(bool)`.
- `kill()` — sets running = false.

## `spawn_actor` + actor task (`instance_actor.rs`)

```rust
pub fn spawn_actor<H: ProcessHandle>(
    instance_id: InstanceId, handle: H, state: Arc<AppState>
) -> InstanceHandle
```

Spawns a `tokio::spawn` loop that:

1. Selects on `stdout_rx.recv()` and `cmd_rx.recv()`.
2. On stdout line: broadcasts `Event::InstanceOutput`; if line contains `"Done ("` → sets status to `Running`.
3. On `ActorCmd::GracefulStop` → sets `Stopping`, sends `"stop"` to stdin, drains stdout 30s, kills if timeout.
4. On `ActorCmd::Kill` → calls `handle.kill()`, breaks.
5. On process exit: determines final status (`Offline` or `Crashed`), broadcasts `StatusChanged`, removes from `AppState.instances`.

## `InstanceHandle`

Stored in `AppState.instances: DashMap<InstanceId, InstanceHandle>`.

```rust
pub struct InstanceHandle {
    pub cmd_tx:     mpsc::Sender<ActorCmd>,
    pub pid:        Option<u32>,
    pub started_at: Instant,
}
```

## `ActorCmd`

```rust
pub enum ActorCmd {
    SendCommand(String),
    GracefulStop,
    Kill,
}
```

## Rules

- `PtySpawner` is the only production spawner — always use `MockSpawner` in tests.
- The actor must not block the Tokio thread — PTY stdout reading runs in a `std::thread`.
- Status updates in `set_status()` use raw `sqlx::query` (ARCH-01 open bug; bypasses `InstanceStore`).
