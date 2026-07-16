# apps/app-frontend
Amberite desktop app frontend. Vue 3 app built with Vite and loaded inside the Tauri shell from `apps/app`.

## File Structure
```text
apps/app-frontend/
  src/
  	package.json            package config
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

## Instance Types

The instance page at `/instance/:id` supports three instance types: `client` for local game instances, `server` for Core-managed servers, and `synced` for synchronized instances. These are not separate pages; they are the same instance page rendered with different state and available features for each type.

Keep reusable backend-facing logic in `@amberite/amberite-api`; this app owns Vue presentation state, UI-only behavior, and platform wiring. Platform integration may wire a real `ConvexClient` subscription into the shared contracts; raw one-shot calls are only for auth bootstrap, debounced search, or callers that cannot subscribe, and mutations converge through the subscription instead of triggering a full refresh.

## Read These When Working On
| Work                                                        | Read                              |
| ----------------------------------------------------------- | --------------------------------- |
| Any UI work (shared layouts, components, or patterns)       | `packages/ui/AGENTS.md`           |
| Modrinth API client usage or shared API types               | `packages/api-client/AGENTS.md`   |
| core and convex communication,                              | `packages/amberite-api/AGENTS.md` |
| Tauri shell behavior, native permissions, or command wiring | `apps/app/AGENTS.md`              |

# ui rules
