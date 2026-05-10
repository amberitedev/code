# Hosting UI — AI Reference

This document describes the server management UI for the Amberite/Modrinth hosting feature,
to enable AI agents to understand, modify and extend it accurately.

---

## How to Browse the UI in Dev

1. Run `pnpm dev` in `apps/frontend/`
2. Log in to modrinth.com (auth is required)
3. Navigate to **http://localhost:3000/hosting/manage/mock**

The `mock` server ID triggers `apps/frontend/src/plugins/server-mock.client.ts`, which
pre-populates the TanStack Query cache with synthetic data so all pages render without a
real Pyro/Archon backend.

---

## Route Map

| URL | File | Description |
|---|---|---|
| `/hosting` | `apps/frontend/src/pages/hosting/index.vue` | Marketing landing page |
| `/hosting/manage` | `apps/frontend/src/pages/hosting/manage/index.vue` | Server list |
| `/hosting/manage/:id` | `apps/frontend/src/pages/hosting/manage/[id].vue` | Root shell (nav + layout) |
| `/hosting/manage/:id` (child) | `…/[id]/index.vue` | Overview tab |
| `/hosting/manage/:id/content` | `…/[id]/content.vue` | Content/mods tab |
| `/hosting/manage/:id/files` | `…/[id]/files.vue` | File manager tab |
| `/hosting/manage/:id/backups` | `…/[id]/backups.vue` | Backups tab |
| `/hosting/manage/:id/options` | `…/[id]/options.vue` | Options shell (sub-sidebar) |
| `/hosting/manage/:id/options` (child) | `…/[id]/options/index.vue` | General — server name, custom URL, icon |
| `/hosting/manage/:id/options/loader` | `…/[id]/options/loader.vue` | Platform/loader/MC version, reset server |
| `/hosting/manage/:id/options/startup` | `…/[id]/options/startup.vue` | Startup command, Java version, JRE vendor |
| `/hosting/manage/:id/options/network` | `…/[id]/options/network.vue` | Subdomain, allocations, DNS records |
| `/hosting/manage/:id/options/properties` | `…/[id]/options/properties.vue` | server.properties editor |
| `/hosting/manage/:id/options/preferences` | `…/[id]/options/preferences.vue` | Per-server localStorage prefs |
| `/hosting/manage/:id/options/billing` | `…/[id]/options/billing.vue` | Billing redirect card |
| `/hosting/manage/:id/options/info` | `…/[id]/options/info.vue` | SFTP credentials + server info |

---

## Architecture

### Layout Hierarchy

```
[id].vue                        — prefetches v0 server data, renders ServersManageRootLayout
  ServersManageRootLayout       — packages/ui/src/layouts/wrapped/hosting/manage/root.vue
    NavTabs                     — Overview | Content | Files | Backups | Options
    <slot>                      — renders the active NuxtPage child
      [id]/index.vue            — Overview tab content
      [id]/content.vue          — Content tab
      [id]/files.vue            — Files tab
      [id]/backups.vue          — Backups tab
      [id]/options.vue          — Options shell (renders ServerSidebar + nested NuxtPage)
        [id]/options/index.vue  — General settings
        [id]/options/loader.vue — Platform settings
        …
```

### Key Providers (dependency injection)

| Provider | File | What it gives |
|---|---|---|
| `injectModrinthServerContext()` | `packages/ui/src/providers/` | `server`, `serverId`, `worldId`, `powerState`, `busyReasons`, `isSyncingContent` |
| `injectModrinthClient()` | `packages/ui/src/providers/` | Typed API client with `.archon`, `.labrinth`, `.kyros`, etc. |
| `injectNotificationManager()` | `packages/ui/src/providers/` | `addNotification({type, title, text})` |
| `injectTags()` | `packages/ui/src/providers/` | `gameVersions`, `loaderNames`, etc. |

All providers are set up inside `ServersManageRootLayout`. Child pages call `inject*` to receive them.

### TanStack Query Keys

| Data | Query key |
|---|---|
| Server (v0) | `['servers', 'detail', serverId]` |
| Server (v1, worlds) | `['servers', 'v1', 'detail', serverId]` |
| Allocations | `['servers', 'allocations', serverId]` |
| Content/addons | `['content', 'list', 'v1', serverId]` |
| server.properties | `['servers', 'properties', 'v1', serverId, worldId]` |
| Startup config | `['servers', 'startup', 'v1', serverId, worldId]` |
| Backup queue | `['backups', 'queue', 'v1', serverId, worldId]` |

---

## Key Components

### `SaveBanner`
Path: `apps/frontend/src/components/ui/servers/SaveBanner.vue`

Sticky floating banner that appears when there are unsaved changes.

```vue
<SaveBanner
  :is-visible="hasUnsavedChanges"
  :server-id="serverId"
  :is-updating="isUpdating"
  :save="saveHandler"
  :reset="resetHandler"
  :restart="true"  <!-- optional: adds 'Save & restart' button -->
/>
```

### `ServerSidebar`
Path: `apps/frontend/src/components/ui/servers/ServerSidebar.vue`

Left sidebar for the Options sub-section showing nav links.

```vue
<ServerSidebar :route="route" :nav-links="navLinks" />
```

`navLinks` shape: `{ icon: Component, label: string, href: string, external?: boolean, shown?: boolean }[]`

### `ServersManageRootLayout` (packages/ui)

The main wrapper. Passes `additionalTabs` to extend the top nav beyond the default 4 tabs.
The Options tab is added via:

```ts
:additional-tabs="[{ label: 'Options', href: `/hosting/manage/${serverId}/options`, icon: SettingsIcon, subpages: ['loader', ...] }]"
```

---

## API Client

The client is obtained via `injectModrinthClient()`. Key namespaces used in hosting:

| Namespace | Used for |
|---|---|
| `client.archon.servers_v0` | get, list, power, updateName, changeSubdomain, getAllocations |
| `client.archon.servers_v1` | get (v1, worlds list), endIntro |
| `client.archon.content_v1` | getAddons, installContent, repair, unlinkModpack |
| `client.archon.properties_v1` | getProperties, patchProperties |
| `client.archon.options_v1` | getStartup, patchStartup |
| `client.archon.backups_queue_v1` | list, create, restore |
| `client.kyros.files_v0` | uploadFile, deleteFileOrFolder |

---

## Mock Data

`apps/frontend/src/plugins/server-mock.client.ts` intercepts all TanStack queries for
server ID `"mock"` and returns synthetic data. To extend the mock (e.g. add more properties
or mods), edit the `mock*` constants at the top of that file.

---

## Amberite Differences from Modrinth

- The repo is a fork. All Amberite-specific code lives in `apps/` or `packages/amberite-lib/`.
- Amberite uses Microsoft auth (not Modrinth auth) via `apps/supabase/functions/microsoft-auth/`.
- The Amberite backend is `apps/core/` (Rust/Axum), not Pyro/Archon.
- The hosting UI currently speaks to Modrinth's Archon API. Future work: swap client to point at `apps/core/`.
