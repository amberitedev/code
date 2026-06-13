# apps/app

Desktop app package. This is the Tauri shell that embeds `apps/app-frontend` and wires it to Rust-side capabilities and internal workspace dependencies.

## Scope

- Native windowing, packaging, updater, capabilities, and Tauri configuration live here.
- Most rendered UI work belongs in `apps/app-frontend`.
- Do not start Tauri, the app, or app dev commands unless the user explicitly asks.

## Read These First

| File                              | Why it matters                              |
| --------------------------------- | ------------------------------------------- |
| `apps/app-frontend/AGENTS.md`     | Frontend UI rules and file structure        |
| `packages/ui/AGENTS.md`           | Shared UI reuse rules                       |
| `packages/amberite-api/AGENTS.md` | Amberite API package guidance               |
| `AGENTS.md`                       | Repo-wide rules and desktop app constraints |

## Internal Dependencies

| Dependency               | Role here                                                                   | Guidance                                                                    |
| ------------------------ | --------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `@modrinth/app-frontend` | Vue frontend rendered inside the Tauri shell                                | Read `apps/app-frontend/AGENTS.md`                                          |
| `@modrinth/app-lib`      | Native Rust-side app logic, data, launcher behavior, and local integrations | No package AGENTS file; do not modify unless explicitly asked by repo rules |
| `@modrinth/daedalus`     | Shared launcher/backend support crate used by the app stack                 | No package AGENTS file                                                      |
| `@modrinth/ui`           | Shared component and layout system used by the frontend                     | Read `packages/ui/AGENTS.md`                                                |
| `@amberite/amberite-api` | Shared Amberite communication layer used across app/core integrations       | Read `packages/amberite-api/AGENTS.md`                                      |
| `@modrinth/api-client`   | Shared Modrinth API client used by frontend-side integrations               | Read `packages/api-client/AGENTS.md`                                        |

## File Structure

```text
apps/app/
  package.json              Tauri scripts and workspace dependencies
  Cargo.toml                Rust crate definition
  tauri.conf.json           Main Tauri runtime/package config
  tauri.*.conf.json         Platform-specific Tauri overrides
  capabilities/             Tauri permission bundles for windows, plugins, and feature access
    core.json               Base app/window/webview permissions
    plugins.json            Plugin permissions, HTTP allowlist, FS scope, and command bundles
    ads.json                Ads-specific capability bundle
    updater.json            Updater-specific capability bundle
  src/                      Rust-side app shell code
    main.rs                 Tauri entrypoint, plugin wiring, commands, lifecycle hooks
    api/                    Tauri command modules exposed to the frontend
    macos/                  macOS-specific deep link helpers
    error.rs                Error display/serialization for Tauri return values
    updater_impl*.rs        Real/no-op updater implementations by feature flag
  icons/                    App icons and packaging assets
```

## General Rules

- Keep changes scoped. Most UI rendering belongs in `apps/app-frontend`; this package is mainly shell, native integration, permissions, and command wiring.
- All reusable Copal logic must live in `@amberite/amberite-api`; app code should call that bridge instead of composing Core workflows locally.
- When changing Tauri commands, check both the Rust module in `src/api/` and any capability file that grants access to it.
- Be careful with capability changes: they define what the webview is allowed to do, including network access, filesystem scope, and plugin usage.
- Avoid broadening HTTP allowlists or filesystem scope unless the task clearly requires it.
- Preserve platform guards and feature flags; this package has macOS-specific logic and updater-on/updater-off code paths.
