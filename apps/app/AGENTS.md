# Amberite Desktop App

Fork of Modrinth App with Amberite-specific extensions. Goal: minimize merge conflicts when upstream updates.

## Project Structure

```
apps/app/
├── frontend/              # Vue 3 UI forked from Modrinth
│   ├── src/              # Application source
│   ├── vite.config.ts    # Build config with aliases
│   └── package.json      # Dependencies, scripts
├── backend/               # Amberite's Rust library (separate from Theseus)
│   ├── src/              # lib.rs, error.rs
│   └── Cargo.toml        # Dependencies: tauri, serde, tokio, tracing
├── tauri/                 # Tauri shell forked from Modrinth
│   ├── src/              # main.rs, api/, macos/
│   ├── capabilities/     # Tauri security capabilities
│   ├── icons/            # App icons
│   ├── tauri.conf.json   # Tauri config
│   └── Cargo.toml        # Dependencies: theseus, amberite-backend, tauri plugins
└── package.json           # Workspace scripts
```

Each subdirectory has its own AGENTS.md with specifics.

## Fork Architecture

### Core Dependencies
- **theseus**: Git dependency from `modrinth/code` - Modrinth's launcher library
- **amberite-backend**: Path dependency - Amberite-specific logic (owned by us)
- Frontend UI packages: Aliased `@modrinth/*` → `@amberite/*` via vite.config.ts

### Merge Conflict Strategy
When Modrinth updates upstream, merging forked files is painful. Approach:

1. **Theseus wrappers** (`tauri/src/api/*.rs`): Keep minimal - just pass through. No custom logic.
2. **Amberite namespace** (`tauri/src/api/amberite/`): All custom behavior here. Calls `amberite-backend`.
3. **Backend library** (`backend/src/`): New modules for Amberite features. Safe from upstream merges.
4. **Frontend helpers**: Use `plugin:amberite|*` namespace for custom features.

The `api/amberite/` namespace is ours - upstream won't touch it. This is the only safe place for modifications in the forked Tauri code.

### Limitation
You cannot fully "swap out" Theseus functions without modifying `main.rs` (plugin registration). The `tauri/` directory is forked - upstream merges will conflict. Minimize changes outside `api/amberite/`.

## Tauri Plugin System

Functions exposed to frontend are called "commands" (Tauri terminology). A Rust function marked with `#[tauri::command]` becomes callable from JavaScript via `invoke()`.

Plugins group commands under a namespace:

```js
invoke('plugin:utils|get_os')         // Theseus wrapper (forked)
invoke('plugin:amberite|hello')       // Amberite command (ours)
invoke('initialize_state')            // Top-level (no namespace)
```

Registration in `tauri/src/main.rs`:
```rust
.plugin(api::auth::init())      // creates plugin:auth| namespace
.plugin(api::amberite::init())  // creates plugin:amberite| namespace
```

## Tauri Configuration

`tauri/tauri.conf.json` settings:
- **identifier**: `com.amberite.app`
- **productName**: `Amberite`
- **deep-link schemes**: `modrinth://` (kept for .mrpack compatibility)
- **fileAssociations**: `.mrpack` files open the app
- **beforeDevCommand**: Runs frontend via turbo
- **CSP**: Allowlist for Modrinth services, Stripe, analytics, textures.minecraft.net
- **windows**: Hidden initially, custom title bar (no decorations), min size 1100x700

## State Initialization Flow

App starts with hidden window to prevent visual flash:

1. `main.rs` registers plugins, creates hidden window
2. Frontend calls `initialize_state()` → Theseus State initializes
3. State sets up directories, asset scopes, file scopes
4. Frontend calls `show_window()` → Window revealed
5. Deep link handlers ready for `.mrpack` and `modrinth://` URLs

## Deep Link Handling

External triggers that open the app:

- **File association**: `.mrpack` files → opens app, triggers modpack install flow
- **URL scheme**: `modrinth://` URLs → deep link events (e.g., install project)

Flow: `handle_command()` parses → emits event → frontend `command_listener()` receives.

## Frontend-Backend Communication

### Invoke (command call)
```js
import { invoke } from '@tauri-apps/api/core'
const result = await invoke('plugin:amberite|command', { arg: value })
```

### Events (push notifications)
```js
// Backend emits
app.emit('event-name', payload)

// Frontend listens
import { listen } from '@tauri-apps/api/event'
const unlisten = await listen('event-name', (e) => handle(e.payload))
```

Events: progress bars, warnings, auth changes, command handling, download progress.

## Error Handling

Theseus errors wrapped in `TheseusSerializableError` for JS serialization:

```rust
pub enum TheseusSerializableError {
    Theseus(#[from] theseus::Error),
    IO(#[from] std::io::Error),
    Tauri(#[from] tauri::Error),
}
```

Frontend receives `{ field_name: "Theseus", message: "..." }`. The `error.js` store shows modal.

## Update Mechanism (Windows Only)

- `updater` feature flag controls built-in updater
- Auto-download updates, install on exit or restart
- Metered connection detection: skips auto-download, prompts user
- Pending update toast on next launch

## Security

- Asset protocol: scoped to cache/profile directories only
- CSP: specific external services allowed (no wildcard)
- File access: requires user dialog confirmation
- Credentials: stored via Theseus credential manager (OS-specific secure storage)

## What NOT To Do

- Don't add logic to Theseus wrappers (`api/*.rs`) - keep pass-through only
- Don't use Theseus namespaces for Amberite features (use `plugin:amberite|`)
- Don't import `@modrinth/*` directly in frontend (use aliases)
- Don't modify `main.rs` unless registering new Amberite plugin
- Don't skip hidden-window initialization pattern

