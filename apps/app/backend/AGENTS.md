# Amberite Backend

Rust library for Amberite-specific features. Separate from Theseus to avoid merge conflicts.

## Source Structure

```
src/
├── lib.rs    # Public API, placeholder functions
└── error.rs  # BackendError enum
```

Currently minimal - placeholder. Will grow with Amberite features.

## Dependencies

From `Cargo.toml`:
- `tauri` - Tauri integration
- `serde`, `serde_json` - Serialization
- `thiserror` - Error derive
- `tokio` - Async runtime
- `tracing` - Logging

## Adding Functions

1. Add function in `lib.rs`:

```rust
pub fn my_feature() -> Result<MyData> {
    // implementation
}

#[derive(Serialize, Deserialize)]
pub struct MyData {
    pub field: String,
}
```

2. Create Tauri command in `tauri/src/api/amberite/mod.rs` that calls this function.

3. Register command in `api/amberite/init()`.

## Error Handling

Use `BackendError` from `error.rs`:

```rust
pub enum BackendError {
    Config(String),
    Io(std::io::Error),
    Serialization(serde_json::Error),
    Async(String),
}
```

Return `Result<T>` (alias for `std::result::Result<T, BackendError>`).

## Dev Commands

From `apps/app/backend/`:
```bash
cargo run      # Build and run library
cargo build    # Build library
cargo test     # Run tests
cargo fmt      # Format code
cargo clippy   # Lint
```

## Integration with Tauri

The Tauri shell (`tauri/`) depends on this via path:

```toml
# tauri/Cargo.toml
amberite-backend = { path = "../backend" }
```

Commands in `tauri/src/api/amberite/` call functions from this library. This separation keeps Amberite logic safe from upstream merges.