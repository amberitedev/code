# packages/api-client

Shared Modrinth API client package.

## Context Loading

- Load `CLAUDE.md` for the full client architecture and endpoint-generation details.

## Use This Package When

- Frontend or app code needs typed Modrinth API access.
- A component/composable currently imports outdated types from `@modrinth/utils` and can move to API-client types instead.
- You are adding or updating calls that should use the shared client rather than ad-hoc fetch logic.

## Rules

- Prefer this package's generated and shared types over `@modrinth/utils` when both are available.
- Do not invent new API wrappers in app code if the client already exposes the endpoint or type you need.
