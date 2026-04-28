# Amberite Bugs

**Last updated:** 2026-04-27

## CRITICAL — Blocks Compilation
- **`mod.rs` duplicate declarations:** `apps/app/tauri/src/mod.rs` lines 5–18 are
  duplicated as lines 20–33; `oauth_utils` declared twice on lines 43–44. App cannot
  compile at all. Fix: remove duplicate lines. See prompt in `.plan/desktop-backend/README.md`.

## amberite-backend
- **Dual error types:** `AmberiteError` in `lib.rs` and `BackendError` in `error.rs`
  are inconsistent. Must be reconciled into single `AmberiteError` in `error.rs`
  before adding new backend modules — MEDIUM

## Architectural (from Cursor analysis)
- **Workspace filter mismatch:** `--filter=@amberite/web` vs `modrinth-app-ui` — HIGH priority
- **Tauri version drift:** CLI 2.5.0, crate 2.8.5, JS 2.10.x — MEDIUM
- **Three Cargo.lock files:** backend, tauri, core — LOW (intentional)
- **Catalog vs workspace styles:** dual dependency style — LOW (cosmetic, will resolve when pnpm-workspace catalog is updated)

## Core (from problem-summary.md)
- **Auth fundamentally broken:** PASETO vs Supabase JWT mismatch — CRITICAL
- **Instance data model incomplete:** missing game_type, version, port, etc. — HIGH
- **Supabase integration missing:** no heartbeat, no JWT validation — HIGH
- **Process kill doesn't work:** ghost servers keep running — HIGH
- **Friend groups tables missing:** only individual friendships exist — MEDIUM
- **Mod sync flow missing:** owner push → member download — MEDIUM

## Version Desynchronization
- **MODRINTH_VERSION.ts stale:** says `v0.13.3`, root file says `v0.13.4` — HIGH (milestone 1)
- **pnpm-workspace.yaml catalog stale:** says `v0.13.1`, should be `v0.13.4` — HIGH (milestone 1)
- **backend/Cargo.toml non-workspace deps:** `tauri = "2.5"`, `thiserror = "1.0"` mismatch with workspace versions (2.8.5, 2.0.17) — MEDIUM (milestone 1)

## Directory Structure
- **app-frontend outside app/:** `apps/app-frontend/` should be `apps/app/frontend/` — HIGH (milestone 1)
- **Tauri files at app root:** `apps/app/src/`, `Cargo.toml`, `tauri.conf.json` etc. should be in `apps/app/tauri/` — HIGH (milestone 1)

## Theseus Namespace
- **Keychain namespace collision:** `"com.modrinth.theseus"` in `legacy_converter.rs` — HIGH (milestone 1, vendor patch)
- **User-Agent header:** `modrinth/theseus` should be `amberite/app` — LOW (milestone 1, vendor patch)

## Quality
- **No CI/CD:** missing GitHub Actions — HIGH (milestone 1: sync-theseus workflow)
- **No real tests:** only vue-tsc typecheck — MEDIUM (planning axum-test + invoke script)
- **Supabase credentials committed:** security risk — SECURITY (rotate immediately)
- **placeholder test message mismatch:** test expects `"Amberite backend initialized"` but we're changing it to `"hello from Amberite"` — MEDIUM (fix test in milestone 1)

## Lodestone Legacy (from old bugs.md)
- Console ANSI colors stripped — MEDIUM
- No setup status check (re-runs first-time setup) — MEDIUM
- playit.gg API outdated — LOW
- UPnP stubbed out — LOW