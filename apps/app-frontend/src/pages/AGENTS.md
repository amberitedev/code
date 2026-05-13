# pages/

Route-mapped views. Each subfolder maps to a route group in `routes.js`. Each folder has an `index.js` that exports named views consumed by `routes.js`.

---

## Route Map

| Route                  | Folder            | Notes                                                 |
| ---------------------- | ----------------- | ----------------------------------------------------- |
| `/`                    | `Index.vue`       | Home                                                  |
| `/worlds`              | `Worlds.vue`      |                                                       |
| `/skins`               | `Skins.vue`       |                                                       |
| `/browse/:projectType` | `Browse.vue`      |                                                       |
| `/library`             | `library/`        | Tabs: Overview, Downloaded, Modpacks, Servers, Custom |
| `/project/:id`         | `project/`        | Description, Versions, Version, Gallery, Changelog    |
| `/instance/:id`        | `instance/`       | Modrinth-style local instance                         |
| `/server/:id`          | `server/`         | **Amberite** — Core-managed server (dedicated server) |
| `/synced/:id`          | `synced/`         | **Amberite** — Core-synced game instance              |
| `/hosting/manage/:id`  | `hosting/manage/` | **Amberite** — hosted (cloud) server management       |
| `/hosting/manage/`     | `Servers.vue`     | Server list                                           |

---

## Amberite-Specific Pages

**`server/`** — dedicated server managed by Amberite Core. `Index.vue` delegates almost everything to `ServersManageRootLayout` from `@modrinth/ui`, injecting navigation callbacks and a `CoreApiClient`. Sub-pages: Overview, Console, Content, Files, Backups.

**`synced/`** — game instance synced by Core. `Index.vue` handles play/stop, Tauri event listeners, and breadcrumbs. Sub-pages are **re-exported from other folders** (see gotchas). `ServerSetup.vue` wraps content that requires Core to be running.

**`hosting/manage/`** — cloud-hosted server. Sub-pages: Overview, Content, Files, Backups.

---

## Route Meta

- `renderMode: 'fixed'` — used on Console and Logs routes; the parent layout switches to fixed-height flex so the terminal fills the viewport without page scroll.
- `useContext` — breadcrumb reads a dynamic name from context (e.g. project title).
- `useRootContext` — same, but from root context (instance name).
- `breadcrumb` with `?Name` prefix — name is resolved dynamically at runtime.

---

## Gotchas

- `synced/index.js` has no own sub-page views — it re-exports from `instance/` (Mods, Files, Worlds, Logs) and `server/` (Console, Backups). Editing a synced sub-page means editing the instance or server view.
- Old `/instance/:id` Overview route is commented out in `routes.js` — the default child (`''`) renders `Mods` instead.
- URL redirect at `/:projectType(mod|plugin|...)/:id` normalizes old Modrinth URLs to `/project/:id`.
- Scroll behavior targets `.app-viewport` element, not `window` — Linux workaround (`routes.js:349`).
