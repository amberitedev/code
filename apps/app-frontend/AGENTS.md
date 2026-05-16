# app-frontend

Vue 3 frontend for the Amberite desktop app — a fork of the Modrinth desktop app with added Amberite Core integration for server management and synced instances.

---

## Structure

```
src/
  adapters/     Tauri bridge (PlatformAdapter singleton)
  components/   App-specific components (ui/ has all of them)
  composables/  Small Vue composables
  directives/   Vue directives (overlay-scrollbars)
  helpers/      All business logic — Tauri invoke wrappers + API calls
  pages/        Route-mapped views
  plugins/      i18n, i18n-debug
  providers/    DI/setup — wires Vue provide/inject at boot
  store/        Pinia stores (breadcrumbs, theme, error, install)
  locales/      i18n translation files
  routes.js     Full route table
  main.js       App bootstrap
```

---

## Key Relationships

- **`adapters/desktop.ts`** — singleton `PlatformAdapter` from `@amberite/api-lib`. Bridges OS keychain session storage, the app-launched Core setup secret, Convex URL, Tauri HTTP fetch (bypasses WebView CORS), and `invoke()` calls to Core. Every page that talks to Core creates a `CoreApiClient` with this adapter.
- **`@amberite/api-lib`** — provides `CoreApiClient` and `PlatformAdapter` types. Used by pages that need Core API access.
- **`@modrinth/ui`** — shared component library. Server management UI (`ServersManageRootLayout`) lives here, not in this package. See `packages/ui/AGENTS.md`.
- **`packages/app-lib/`** — provides Tauri commands. `.env` for this app is read from `packages/app-lib/.env` (loaded manually in `vite.config.ts`), not from this directory.
- **`helpers/`** — all Tauri `invoke()` wrappers and API logic. Pages import from here, not from `@modrinth/ui` data utilities.

---

## What Amberite Added (vs Modrinth fork)

| Area                       | Where                         |
| -------------------------- | ----------------------------- |
| Core-managed server pages  | `pages/server/`               |
| Core-synced instance pages | `pages/synced/`               |
| Hosted server management   | `pages/hosting/manage/`       |
| Tauri/Convex/Core bridge   | `adapters/desktop.ts`         |
| Server install providers   | `providers/server-install.ts` |

Everything else (`Browse`, `Library`, `Project`, `Instance`, `Skins`, `Worlds`) is inherited from Modrinth.

---

## Gotchas

- Dev server must run on port **1420** — Tauri requires a fixed port (`vite.config.ts:73`).
- Env vars are loaded from `packages/app-lib/.env`, not from this directory. If env vars are missing at runtime, check there.
- Env prefix: `VITE_`, `TAURI_`, `MODRINTH_` (see `vite.config.ts:94`).
- Desktop JWT storage is not `localStorage`; use `PlatformAdapter.getCurrentJwt()` / `setCurrentJwt()` so the Tauri OS keychain bridge is used.
- `synced/` pages mostly re-export views from `instance/` and `server/` — see `pages/synced/index.js`.

---

## Navigation

| Need                                        | Go to                          |
| ------------------------------------------- | ------------------------------ |
| Route table                                 | `src/routes.js`                |
| Page views                                  | `src/pages/` → `AGENTS.md`     |
| Business logic / Tauri calls                | `src/helpers/` → `AGENTS.md`   |
| Boot/DI wiring                              | `src/providers/` → `AGENTS.md` |
| App-level state                             | `src/store/`                   |
| App-specific components                     | `src/components/ui/`           |
| Shared UI library (components, layouts, DI) | `packages/ui/` → `AGENTS.md`   |
