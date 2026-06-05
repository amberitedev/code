# apps/app/src

Rust-side Tauri shell source for the desktop app.

## What Lives Here

- `main.rs` builds the Tauri app, registers plugins, wires Tauri commands, and handles app lifecycle events.
- `api/` contains feature-specific Tauri command modules exposed to the frontend.
- `macos/` contains macOS-only deep link handling.
- `error.rs` formats and serializes Rust-side errors so they can cross the Tauri boundary.
- `updater_impl.rs` and `updater_impl_noop.rs` switch updater behavior based on feature flags.

## How The Shell Works

- The frontend in `apps/app-frontend` runs inside the Tauri webview.
- `main.rs` registers Tauri plugins such as HTTP, FS, dialogs, opener, deep link, and window-state.
- It also registers app-specific plugin modules from `api::*`, which are the Rust commands the frontend calls.
- Startup flow includes state initialization, filesystem/asset scope setup, single-instance handling, deep-link handling, and optional updater setup.
- Shutdown flow includes pending skin flushes and optional deferred updater installation logic.

## Main Areas

```text
src/
  main.rs                   App entrypoint and plugin/command wiring
  api/
    mod.rs                  API module registry and serializable error type
    auth.rs                 Auth-related commands
    mr_auth.rs              Modrinth auth commands
    import.rs               Import flows and file/deep-link handling
    logs.rs                 Logging and log access
    jre.rs                  Java runtime management
    metadata.rs             Metadata access
    minecraft_skins.rs      Skin operations and pending flush behavior
    pack.rs                 Pack-related commands
    process.rs              Process lifecycle commands
    profile*.rs             Profile and profile-creation commands
    settings.rs             Settings commands
    tags.rs                 Tag data commands
    cache.rs                Cache commands
    files.rs                Filesystem/file commands
    ads*.rs                 Ads commands and OS-specific occlusion helpers
    friends.rs              Friends commands
    worlds.rs               World-related commands
    utils.rs                Shared utilities and command helpers
    oauth_utils/            OAuth reply helpers and embedded auth page
  macos/
    deep_link.rs            macOS deep-link payload coordination
  error.rs                  Tauri-facing error formatting
  updater_impl*.rs          Feature-flagged updater implementations
```
