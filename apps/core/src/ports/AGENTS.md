# ports/

Trait definitions for dependency inversion. Infrastructure implements these; application calls them.

## Files

| File | Trait(s) | Error type |
|------|----------|------------|
| `instance_store.rs` | `InstanceStore` | `StoreError` |
| `modpack_store.rs` | `ModpackStore` | `StoreError` (shared) |
| `process_spawner.rs` | `ProcessSpawner`, `ProcessHandle` | `SpawnError` |
| `mod.rs` | Re-exports |

## `InstanceStore`

```rust
async fn create(r: &InstanceRecord)
async fn get(id: &InstanceId) -> InstanceRecord
async fn list() -> Vec<InstanceRecord>
async fn list_by_status(status: InstanceStatus) -> Vec<InstanceRecord>
async fn update_status(id: &InstanceId, status: InstanceStatus)
async fn delete(id: &InstanceId)
```

## `ModpackStore`

```rust
async fn save(m: &ModpackManifest)
async fn get_for_instance(instance_id: &str) -> Option<ModpackManifest>
async fn delete_for_instance(instance_id: &str)
```

## `ProcessHandle` + `ProcessSpawner`

```rust
trait ProcessHandle {
    fn send_stdin(&self, line: &str) -> Result<(), SpawnError>;
    fn take_stdout_rx(&mut self) -> Option<mpsc::Receiver<String>>;
    fn is_running(&self) -> bool;
    fn kill(&mut self) -> Result<(), SpawnError>;
    fn pid(&self) -> Option<u32> { None }   // default impl
}

trait ProcessSpawner {
    type Handle: ProcessHandle;
    async fn spawn(command, args, cwd, env) -> Handle;
}
```

## Rules

- Traits only — no implementation here.
- `StoreError` variants: `Sqlx(sqlx::Error)`, `NotFound(String)`, `Parse(String)`.
- `SpawnError` variants: `Failed(String)`, `Io(std::io::Error)`.
