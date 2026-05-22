# helpers/

All business logic. Two kinds of files: Tauri `invoke()` wrappers (talk to Rust) and data/API utilities.

---

## Files

### Tauri Invoke Wrappers

| File          | What it wraps                                           |
| ------------- | ------------------------------------------------------- |
| `state.ts`    | App state init, progress bars, opening command handling |
| `profile.ts`  | Instance CRUD, run/kill, install, path resolution       |
| `process.js`  | Process list, process state by profile path             |
| `jre.js`      | JRE detection, download, auto-install                   |
| `settings.ts` | App-level settings (get/set)                            |
| `tags.js`     | Modrinth tag/category data (cached from backend)        |
| `import.js`   | Modpack/instance import flows                           |

### Data / API

| File               | What it does                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------------ |
| `cache.js`         | User, team, project data from Modrinth API (in-memory cache)                                     |
| `metadata.js`      | Version metadata lookups                                                                         |
| `pack.ts`          | Modpack operations (install, update)                                                             |
| `worlds.ts`        | World listing and management                                                                     |
| `skins.ts`         | Skin upload/fetching via Modrinth API                                                            |
| `friends.ts`       | Friends/social features                                                                          |
| `amberite-auth.ts` | Convex Auth login/refresh bridge; stores Convex JWT + refresh token through Tauri keychain calls |
| `friend-groups.ts` | Amberite Convex friend group helpers, direct calls until api-lib settles                         |
| `convex.ts`        | App-local Convex HTTP query/mutation helper, avoids api-lib churn                                |
| `amberite-core.ts` | Core settings, permission presets, local Core process commands, and direct Core social endpoints |
| `mr_auth.ts`       | Modrinth credentials (stored token, user_id)                                                     |
| `auth.js`          | Modrinth OAuth login flow                                                                        |

### Utilities

| File           | What it does                                                           |
| -------------- | ---------------------------------------------------------------------- |
| `events.js`    | Tauri event listener wrappers (`profile_listener`, `process_listener`) |
| `logs.js`      | Instance log streaming                                                 |
| `utils.js`     | General helpers (`showProfileInFolder`, clipboard, etc.)               |
| `analytics.ts` | `trackEvent()` — fires analytics events                                |
| `ads.js`       | Ad provider integration                                                |
| `types.d.ts`   | Shared TypeScript types (`GameInstance` and others)                    |

### Subdirectories

| Dir          | What it contains                                                                    |
| ------------ | ----------------------------------------------------------------------------------- |
| `rendering/` | `batch-skin-renderer.ts` — canvas-based skin preview renderer                       |
| `storage/`   | `head-storage.ts`, `skin-preview-storage.ts` — IndexedDB caches for rendered images |

---

## Key Types

`types.d.ts` defines `GameInstance` — the central type for local instances. Used throughout pages and components. Also defines `LoadingBar`, `OpeningCommand`, and others.

---

## Gotchas

- `mr_auth.ts` and `auth.js` are both auth-related but distinct: `mr_auth.ts` reads stored Modrinth credentials (token/user_id from Rust), `auth.js` handles the OAuth flow.
- `convex.ts` calls Convex HTTP endpoints directly (`/api/query`, `/api/mutation`) using path strings like `friends:ensureSocialProfile`; it relies on the JWT from `PlatformAdapter.getCurrentJwt()`. Core setup derives the owner ID from `auth:currentUser`, not from a manually typed field.
- `amberite-auth.ts` is the only frontend entry for Convex Auth. Non-interactive calls silently refresh with the stored refresh token; Core pages pass `interactive: true` to launch Microsoft OAuth through the Tauri loopback bridge when needed.
- `events.js` listeners (`profile_listener`, `process_listener`) return an unlisten function — **always call it in `onUnmounted`** or the listener leaks.
- `state.ts`'s `get_opening_command()` must be called only once, after the app is fully booted (per the doc comment).
