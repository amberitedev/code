# Application Layer

## What This Is

Orchestrates domain logic using infrastructure ports. Services, actors, and dependency injection — no HTTP or database code.

## Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports |
| `registry.rs` | ServiceRegistry for DI |
| `auth_service.rs` | Login, registration, token validation |
| `instance_service.rs` | Manages active instance actors via DashMap |
| `instance_actor.rs` | Isolated actor per instance |
| `macro_engine.rs` | JavaScript macro execution |

## Core Pattern: Actor Model

Each running server has its own `InstanceActor` in a Tokio task:

```rust
pub enum InstanceCommand {
    Start, Stop { graceful: bool }, Kill, SendCommand(String)
}

pub enum InstanceEvent {
    Started, Stopped { exit_code: i32 }, Error(String), ConsoleLine(String)
}
```

**Why:** Zero lock contention. `DashMap<InstanceId, Sender<Command>>` routes requests.

## File Details

### `registry.rs`
Narrow DI instead of God-object:
```rust
pub struct ServiceRegistry {
    pub auth_service: Arc<AuthService>,
    pub instance_service: Arc<InstanceService>,
    pub macro_engine: Arc<MacroEngine>,
}
```

### `auth_service.rs`
| Method | Purpose |
|--------|---------|
| `authenticate(username, password)` | Verify with Argon2, return token |
| `register(username, password)` | Hash password, create user |
| `validate_token(token)` | Parse and check expiration |

**Token format:** `user_id:expires_at:uuid`

### `instance_service.rs`
| Method | Purpose |
|--------|---------|
| `start_instance(id)` | Spawn actor, send Start |
| `stop_instance(id, graceful)` | Send Stop to actor |
| `kill_instance(id)` | Send Kill (force) |
| `send_command(id, cmd)` | Send console command |
| `is_running(id)` | Check if active |

**Internal:** `DashMap<InstanceId, mpsc::Sender<InstanceCommand>>`

### `instance_actor.rs`
One actor per server. Runs in Tokio task:
```rust
pub async fn run(mut self) {
    loop {
        // Handle Start, Stop, Kill, SendCommand
    }
}
```

**State:** `Option<Box<dyn ProcessHandle>>` — the OS process.

**Lifecycle:**
1. Service creates actor, spawns task
2. Actor receives Start, calls `OSProcessManager::spawn()`
3. Process stored, `Started` event broadcast
4. Stop/Kill drops process, `Stopped` broadcast

### `macro_engine.rs`
```rust
pub async fn execute(&self, code: &str) -> Result<(), MacroEngineError>
```
Executes JavaScript via Deno (infrastructure provides runtime).

## Error Types

| Error | Location | Variants |
|-------|----------|----------|
| `AuthError` | `auth_service.rs` | InvalidCredentials, UserNotFound, TokenGeneration, PasswordHashing |
| `InstanceServiceError` | `instance_service.rs` | NotFound, AlreadyRunning, NotRunning |
| `MacroEngineError` | `macro_engine.rs` | Execution |

## Testing

Services depend on ports — use mocks:
```rust
let mock_os = Arc::new(MockProcessSpawner::new());
let service = InstanceService::new(repo, mock_os);
service.start_instance(id).await.unwrap();
```
