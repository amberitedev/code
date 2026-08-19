# Amberite

Amberite is a self-hosted modded Minecraft launcher and server manager for private friend groups. A
Rust server called Copal runs on a computer controlled by the group, and the desktop app is built on
top of the Modrinth App.

You can think of Amberite as Modrinth with private, self-hosted group servers built into it. Owners
and Admins manage the server; everyone else installs, updates, and plays.

## What makes Amberite special?

These are the parts of Amberite we should never compromise on.

### 1. Easy to use

### 2. Modrinth is the foundation

<!-- Ilai will write this section. -->

## A note from Ilai

I like ambitious ideas, simple systems, and software that feels obvious. Do not preserve complexity just because it already exists. Do not introduce machinery because it looks architecturally impressive. Understand the real constraint, then fight for the smallest model that makes the correct behavior unsurprising.

Channel both "measure twice, cut once" and "yagni". Fight scope creep. Try to honor the dev's intent in both a minimal and realistic fashion.

The rest of this document is meant to help you navigate the codebase and make changes effectively. Think of these instructions less as "hard rules", more as "good defaults". The developer's preferences should be able to override anything here.

## A small glossary

We need to use the same language when talking about Amberite:

- **you** means the agent reading this file and changing Amberite.
- **we**, **us**, and **maintainers** mean Ilai and the people building Amberite.
- **user** means a person using Amberite.
- **App** means the Amberite desktop launcher and management app.
- **Copal** / **Core** is the a server manager users install.
- **group** means the private friend group connected to a Core.
- **snapshot** means the versioned client state published for a group server.
- **Amberite identity** means the product identity anchored to a verified Minecraft account.

Full glossary with file links: `docs/internals/glossary.md`

## Hit every surface

The most common Amberite defect is a change that works in the one place the agent edited and misses
the rest of the product. Before calling product work done, walk this list and say which entries
applied:

- **Existing product.** Before creating a page, component, or flow, find the existing Amberite and
  Modrinth implementation. Agents repeatedly invented replacements for UI and behavior that already
  existed.
- **User path.** A backend capability that users cannot reach through the real App is internal work,
  not a finished product feature. Complete the path the affected user actually follows.
- **Roles.** Owner, Admin, and Limited are different users. Hiding a control is not authorization;
  the backend that performs the operation must enforce the same decision.
- **Instance types.** Client, server, and synced instances are not interchangeable. Decide which
  side owns the change and how the other side observes it.
- **Backend boundary.** Amberite-facing frontend workflows go through `@amberite/amberite-api`.
  Do not rebuild a multi-step workflow in a Vue component or add another one-off backend path.
- **States.** Loading, empty, offline, unauthorized, failure, retry, and recovery matter when they
  can occur. If you add a way in, add the applicable way out and a way to see the current state.
- **Shared UI.** Start at the page or caller, then local composition and wrappers. Change
  `packages/ui` only when the fix is genuinely shared.
- **Products.** The Desktop App is the server-management surface. Do not add the same management
  feature to the Website merely because both products share code.

## Dev servers

- Use pnpm. Never use npm or Yarn.
- Do not start a dev server unless the developer explicitly asks. Assume the existing environment is
  already running.
- Do not run builds unless the developer explicitly asks.
- Do not kill processes by name or pattern. Stop only an exact process started for the current task
  or one the developer identifies.

Development setup and state handling belong in `docs/internals/scripts.md`.

## Verifying

- Use the smallest proof that demonstrates the affected behavior.
- Prefer checks scoped to the package and files changed. Do not run the entire repository suite
  unless asked.
- Do not run `prepr` unless the developer asks or is preparing a pull request. In the latter case,
  ask whether they want it run.
- Do not claim a build, test, or manual path passed when it was not run.
- Review the final diff and run `git diff --check` before handoff.
- User-visible UI should be checked through the real path when the developer requests browser or
  computer verification.

## Pull requests

- Never create or publish a pull request unless the developer explicitly asks.
- Use conventional titles in plain language.
- Keep one concern per pull request.
- Start the body with the problem, then explain the solution and focused verification.
- UI changes need before-and-after images. Motion or timing changes need a short video.
- Do not include unrelated working-tree changes.

## How it works

The desktop frontend is a Vue app inside a Tauri shell. Amberite-specific frontend workflows live in
`@amberite/amberite-api`, which connects the App and Website to Convex, Copal, and the realtime
Worker. Convex holds durable identity and social state, Copal runs and manages the Minecraft
servers, and realtime carries short-lived presence.

Architecture overview with source links: `docs/internals/overview.md`

## Where code lives

- `apps/app-frontend` - Vue desktop product UI and inherited Modrinth launcher pages.
- `apps/app` - Tauri shell, native commands, capabilities, and process integration.
- `apps/core` - Copal, the Rust server manager.
- `apps/frontend` - Website and inherited Modrinth web surface.
- `convex` - durable Amberite identity, social, group, and cloud state.
- `apps/realtime` - short-lived presence through Cloudflare Workers and Durable Objects.
- `packages/amberite-api` - shared Amberite contracts, clients, and backend-facing workflows.
- `packages/ui` - shared Modrinth and Amberite UI.
- `packages/api-client` - typed Modrinth API client.
- `packages/app-lib` - inherited launcher and platform library. Do not modify it unless explicitly
  asked.
- `apps/labrinth` - inherited Modrinth API service.
- `docs/internals` - architecture, glossary, and focused maintainer documentation.

## Taste

- Complexity belongs at the adapter boundary. Backend-facing workflows stay in
  `@amberite/amberite-api`; UI stays dumb.
- Preserve upstream behavior until Amberite has a product reason to differ.
- Use existing Modrinth UI and nearby patterns before creating new components or visual systems.
- Inferred types over annotations. Never use TypeScript `any`; use `unknown` and narrow it safely.
- Use named exports, tabs for indentation, and the `@` alias for `src` imports.
- Required configuration must fail clearly when missing or invalid. Do not hide configuration
  mistakes behind fallbacks.
- Comments describe how a thing is used or why a constraint exists. Do not narrate ordinary code.
- If a rule here fights the task, say so clearly and get developer approval before breaking it.

## Additional tips

- Product decisions and tasks live in Notion. Read the exact linked page instead of searching for an
  old repository specification.
- Do not read `.plan/` files for orientation. Read a specific plan only when the developer names it.
- Preserve unrelated working-tree changes. Existing changes belong to the developer unless the task
  proves otherwise.
