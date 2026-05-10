# ports/

Port trait definitions for dependency inversion. Infrastructure implements these; application calls them. No implementation code lives here.

## Files

| File | Trait(s) | Error type |
|------|----------|------------|
| `instance_store.rs` | `InstanceStore` | `StoreError` |
| `modpack_store.rs` | `ModpackStore` | `StoreError` (shared type) |
| `process_spawner.rs` | `ProcessSpawner`, `ProcessHandle` | `SpawnError` |
| `mod.rs` | Re-exports |

## `InstanceStore`

```rust
#[async_trait]
trait InstanceStore: Send + Sync + 'static {
    async fn create(&self, record: &InstanceRecord) -> Result<(), StoreError>;
    async fn get(&self, id: &InstanceId) -> Result<InstanceRecord, StoreError>;
    async fn list(&self) -> Result<Vec<InstanceRecord>, StoreError>;
    async fn list_by_status(&self, status: InstanceStatus) -> Result<Vec<InstanceRecord>, StoreError>;
    async fn update_status(&self, id: &InstanceId, status: InstanceStatus) -> Result<(), StoreError>;
    async fn delete(&self, id: &InstanceId) -> Result<(), StoreError>;
}
```

Concrete impl: `infrastructure::db::instance_repo::InstanceRepo`.

## `ModpackStore`

```rust
#[async_trait]
trait ModpackStore: Send + Sync + 'static {
    async fn save(&self, manifest: &ModpackManifest) -> Result<(), StoreError>;
    async fn get_for_instance(&self, instance_id: &str) -> Result<Option<ModpackManifest>, StoreError>;
    async fn delete_for_instance(&self, instance_id: &str) -> Result<(), StoreError>;
}
```

Concrete impl: `infrastructure::db::modpack_repo::ModpackRepo`.

## `ProcessHandle` + `ProcessSpawner`

```rust
trait ProcessHandle: Send + 'static {
    fn send_stdin(&self, line: &str) -> Result<(), SpawnError>;
    fn take_stdout_rx(&mut self) -> Option<mpsc::Receiver<String>>;
    fn is_running(&self) -> bool;
    fn kill(&mut self) -> Result<(), SpawnError>;
    fn pid(&self) -> Option<u32> { None }   // default impl
}

#[async_trait]
trait ProcessSpawner: Send + Sync + 'static {
    type Handle: ProcessHandle;
    async fn spawn(&self, command: &str, args: &[&str], cwd: &Path, env: &[(&str, &str)])
        -> Result<Self::Handle, SpawnError>;
}
```

Concrete impls: `PtySpawner` (production) and `MockSpawner` (tests).

## `StoreError`

```rust
enum StoreError {
    NotFound(String),
    Database(sqlx::Error),
    Parse(String),
}
```

## `SpawnError`

```rust
enum SpawnError {
    Io(std::io::Error),
    Failed(String),
}
```

## Rules

- Traits only — no implementation or SQL here.
- `Send + Sync + 'static` bounds are required for storage in `Arc<AppState>`.
- `StoreError` is shared across both store traits (defined once in `instance_store.rs`).
