# apps/frontend

Nuxt website frontend.

Amberite backend profile fields intentionally mirror Modrinth profile naming where practical (`id`, `username`, `avatar_url`, `bio`, `created`). Amberite profile, social, and Core-list reads are authenticated viewer-safe data, not anonymous public APIs; website pages must require login before calling them.

## Context Loading

- Load `CLAUDE.md` for architecture and package-specific conventions.
- Load `packages/ui/AGENTS.md` when touching shared UI or considering a reusable component.

## UI Routing

- Website-only UI belongs in `src/components/`.
- Reusable UI that could serve both the website and app belongs in `packages/ui`.
- Do not duplicate shared components locally when `@modrinth/ui` already has the needed pattern.
