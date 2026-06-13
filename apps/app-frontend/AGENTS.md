# apps/app-frontend

Amberite desktop app frontend. Vue 3 app built with Vite and loaded inside the Tauri shell from `apps/app`.

## What This Is

- Desktop app UI, routing, providers, and app-side composables.
- Built on top of the existing Modrinth App frontend stack.

## Build Context

- Entry: `src/main.js`
- App shell: `src/App.vue`
- Routes: `src/routes.js`
- Package config: `apps/app-frontend/package.json`
- The full desktop app is launched through root app commands such as `pnpm app:dev`.
- Do not start the app, dev server, or build unless the user explicitly asks.

## File Structure

```text
apps/app-frontend/
  src/
    main.js                 Vue bootstrap
    App.vue                 Root app shell
    routes.js               Route definitions
    pages/                  Route-level screens
    components/
      ui/                   App-specific UI wrappers and desktop-only components
      core/                 Core-server-specific UI
    composables/            Reusable Vue logic
    providers/              Dependency injection and app setup
    helpers/                Tauri-facing helpers and app services
    store/                  Shared frontend state
    assets/                 Stylesheets and static assets
```

## Page Vocabulary

Routes are defined in `src/routes.js`. Use this section to resolve app page names that are easy to confuse; do not treat it as a full route inventory.

| Name | Means | Primary file(s) |
| --- | --- | --- |
| Core | Core management at `/core`, including nested Core screens such as access, roles, rules, and activity panels. | `src/pages/core/Index.vue`, `src/components/core/` |
| Servers / hosting | Hosted server list and management entry point at `/hosting/manage/`. | `src/pages/Servers.vue` |
| Hosted server detail / instance hosting | One hosted server at `/hosting/manage/:id`, including overview, content, files, backups, and access child pages. | `src/pages/hosting/manage/`, `packages/ui/src/layouts/wrapped/hosting/manage/` |
| Client instance | Local client instance at `/instance/:id` when `profile_type === 'client'`, including content, files, worlds, and logs child pages. | `src/pages/instance/InstanceRouter.vue`, `src/pages/instance/Index.vue` |
| Server instance | Core-managed local server instance at `/instance/:id` when `profile_type === 'server'`, or when no app-lib profile exists and the route id is treated as a Core server id. | `src/pages/instance/ServerIndex.vue`, `src/pages/instance/Server*.vue`, `src/pages/instance/server/` |
| Synced instance | Synced instance at `/instance/:id` when `profile_type === 'synced'`. | `src/pages/instance/synced/` |

A user may combine these names with a nested screen or tab name. Resolve that as the named page plus the relevant child screen, such as Core page rules, server instance content, or hosted server files.

## Read These When Working On

| Work | Read |
| --- | --- |
| Shared UI, layouts, or reusable components | `packages/ui/AGENTS.md` |
| Modrinth API client usage or shared API types | `packages/api-client/AGENTS.md` |
| Copal communication, transports, or client wiring | `packages/amberite-api/AGENTS.md` |
| Tauri shell behavior, native permissions, or command wiring | `apps/app/AGENTS.md` |

## UI Rules

- Default to existing `@modrinth/ui` components, layouts, and patterns.
- Assume the component you need already exists in `packages/ui` until proven otherwise.
- Do not invent new button styles, cards, inputs, tabs, toggles, modals, dropdowns, tables, settings rows, or page shells if a shared one already exists.
- Do not introduce a new font, spacing system, radius system, color palette, or visual language.
- Do not use hardcoded hex colors or one-off Tailwind color choices for product UI.
- Keep custom CSS rare and use existing shared component props, tokens, and utilities first.
- Use local app components in `src/components/ui/` only when the behavior is app-specific and not suitable for the shared library.

## Component Search Rule

Before creating any new UI:

1. Search `packages/ui/src/components`.
2. Search `packages/ui/src/layouts`.
3. Read `packages/ui/AGENTS.md` for the shared component catalog and guidance.
4. Search `src/components/ui` for an existing app-specific wrapper.
5. Search nearby pages/routes for the same pattern.

If a shared component or layout can be composed to do the job, use it.

Only create a new component when the existing shared library and local app wrappers truly cannot express the needed behavior.

If you add a new shared UI component or materially change what an existing one is for, update `packages/ui/AGENTS.md` so the component catalog stays current.
