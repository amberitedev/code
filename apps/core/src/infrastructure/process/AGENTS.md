# src/infrastructure/process — Process spawning and per-instance actor

PTY-based process spawning, per-instance async actor, and a mock spawner for tests. See `ports/process_spawner.rs` for the trait definitions.

## File structure

```
process/
  mod.rs              — re-exports: pty_spawner, mock_spawner, instance_actor
  pty_spawner.rs      — PtySpawner + PtyHandle: real PTY-backed process spawning
  mock_spawner.rs     — MockSpawner + MockHandle: in-memory fake for tests
  instance_actor.rs   — ActorCmd, InstanceHandle, spawn_actor: per-instance async actor
```

## pty_spawner.rs

`PtySpawner` opens a PTY pair via `portable_pty::native_pty_system()` and spawns the command on the slave side. The master side gives a writer (for stdin) and a reader (for stdout).

**Stdout reading uses a blocking OS thread** (`std::thread::spawn`): `BufReader::lines()` is a blocking iterator. To bridge this to async, a `std::thread` reads lines and forwards them into an `mpsc::Sender<String>` with `blocking_send`. One OS thread is created per running server instance. These threads self-terminate when the reader closes (process exit or PTY hangup).

**PTY size**: hardcoded to 50 rows × 200 columns. Minecraft's output is unaffected, but ANSI escape sequences for terminal width may behave unexpectedly if the server queries terminal dimensions.

`PtyHandle` wraps the writer and child in `Arc<Mutex<>>` for use across async contexts. `pid()` returns the OS PID used by `stats_service`.

## mock_spawner.rs

`MockSpawner` creates an in-memory channel: the `spawn()` method returns a `MockHandle` where `stdin_tx` and the internal `stdout_rx` share the same channel. This means:
- Anything written via `send_stdin` appears as stdout output in the actor.
- `feed_output` (called from tests) also writes to the same channel, simulating server output lines.

This design lets test code both observe what commands were sent and inject mock server output.

`kill()` sets `running` to `false` via `AtomicBool` — this causes the actor's `is_running()` check to break the event loop, simulating a process exit.

`MockHandle` does not have a `pid()` — returns `None`. Stats tests cannot use a `MockSpawner`-backed instance.

## instance_actor.rs

`spawn_actor` launches a `tokio::spawn` task that owns the `ProcessHandle`. It returns an `InstanceHandle` immediately.

**Actor loop** (`run_actor`):
```
tokio::select! {
    stdout line → broadcast Event::InstanceOutput; detect "Done (" → set Running status
    actor command → SendCommand / GracefulStop / Kill
    else (both channels closed) → break
}
if !handle.is_running() → break
```

**"Done (" detection**: Minecraft prints `Done (Xs)! For help, type "help"` when the server is ready. The actor checks for `"Done ("` substring to transition from `Starting` to `Running`.

**GracefulStop**: sends `"stop"` to stdin, then calls `drain_until_closed` with a 30-second timeout. If the stdout channel does not close in 30 seconds (server did not exit), falls through to `handle.kill()`.

**Exit handling**: after breaking from the loop, the actor checks `handle.is_running()`. If still running (shouldn't happen after kill), it kills and marks as `Crashed`. If not running, marks as `Offline`. Then calls `state.instances.remove(&instance_id)` — this removal is the signal that the instance is fully stopped.

**`set_status` (local)**: the actor has its own private `set_status` function (not the service-layer one) that updates the DB via `instance_store.update_status()` and broadcasts `Event::StatusChanged`. This mirrors `instance_status_service::set_status` but is scoped to the actor.

## Gotchas

- **"Done (" detection is fragile**: any server mod or plugin that prints `"Done ("` before the server is actually ready could cause a premature `Running` status. In practice this is rare.
- **Graceful stop path**: the 30s timeout is in the actor, not the service. `stop_instance` in `instance_status_service` sends `GracefulStop` and returns immediately — it does NOT await the process exit. The status transition to `Offline` happens asynchronously inside the actor.
- **Crashed vs Offline**: the actor sets `Crashed` only if `handle.is_running()` is true at exit — but kill was just called, so `is_running()` is false immediately. The `Crashed` branch is nearly unreachable; a server that crashes on its own shows as `Offline`.
