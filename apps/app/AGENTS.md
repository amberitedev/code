# Aamberite app

This is the main product of Amberite. This directory contains the Tauri shell, which embeds `apps/app-frontend/` in a WebView, registers command plugins, manages the window lifecycle, and bridges the frontend to `theseus` (Minecraft engine) and `amberite-lib` (Core integration).

## Context
	Refer to the appropriate AGENTS.md file for the specific project you are working on.
| Subproject        | AGENTS.md                                | Description                                                        |
| ----------------- | ---------------------------------------- | ------------------------------------------------------------------ |
| `app-lib`         | `packages/app-lib/AGENTS.md`             | Main app backend (Rust), Minecraft launcher engine                 |
| `app-frontend`    | `apps/app-frontend/AGENTS.md`            | Desktop app frontend                                               |
| `ui`              | `packages/ui/AGENTS.md`                  | Shared UI components and layout system                             |
| `amberite-api`    | `packages/amberite-api/AGENTS.md`        | Shared API client for app, core, and integrations                  |
| `amberite-lib`    | `packages/amberite-lib/AGENTS.md`        | Core process, settings, and session storage logic                  |

## File Tree

```
apps/app/
  src/
    main.rs                          — Tauri builder, plugin registration, window lifecycle
    error.rs                         — Tracing span-aware error display utility
    macos/
      mod.rs                         — macOS module gate
      deep_link.rs                   — Deep link handling; mutex-latched payload for pre-init events
    api/
      mod.rs                         — Plugin error type, serialization macro, `Result<T>`
      auth.rs                        — MSA/Xbox auth plugin
      mr_auth.rs                     — Modrinth OAuth plugin
      cache.rs                       — Asset/icon cache plugin (macro-generated commands)
      pack.rs                        — Modpack install/export plugin
      process.rs                     — Minecraft process launch/kill/status plugin
      profile.rs                     — Mod profile CRUD plugin
      profile_create.rs              — Profile creation wizard plugin
      settings.rs                    — App settings read/write plugin
      tags.rs                        — Modrinth tag data plugin
      metadata.rs                    — Game version/loader metadata plugin
      jre.rs                         — Java runtime detection/management plugin
      logs.rs                        — Game log access plugin
      import.rs                      — Import from other launchers plugin
      files.rs                       — Filesystem operations plugin
      friends.rs                     — Modrinth friends/social plugin
      worlds.rs                      — Minecraft world management plugin
      minecraft_skins.rs             — Skin management plugin
      ads.rs                         — Ad integration plugin
      ads-init.js                    — Ad init script bundled with ads plugin
      utils.rs                       — Deep link / URL command handler plugin
      amberite/
        mod.rs                       — Core bridge plugin (session, setup secret, health, URL helpers)
      oauth_utils/
        mod.rs                       — OAuth auth-code reply server module gate
        auth_code_reply.rs           — Loopback HTTP server for OAuth redirects
        auth_code_reply/
          (impl files)
    updater_impl.rs                  — Update download/install (feature-gated)
    updater_impl_noop.rs             — Stub when `updater` Cargo feature is off
  capabilities/
    ads.json                         — Tauri capability group: ads permissions
    core.json                        — Tauri capability group: Core/HTTP permissions
    plugins.json                     — Tauri capability group: plugin command permissions
    updater.json                     — Tauri capability group: updater permissions
  build.rs                           — Build script (advertises permissions)
  Cargo.toml                         — Rust crate manifest
  tauri.conf.json                    — Product name `Amberite`, identifier `amberite`, `visible: false`
  tauri.macos.conf.json              — macOS platform overrides
  tauri.linux.conf.json              — Linux platform overrides
  tauri.no-hmr.conf.json             — HMR-disabled platform overrides
  tauri-release.conf.json            — Release build overrides
```
