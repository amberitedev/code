# Amberite Bugs

**Last updated:** 2026-04-19

## Architectural (from Cursor analysis)
- **Workspace filter mismatch:** `--filter=@amberite/web` vs `modrinth-app-ui` — HIGH priority
- **Tauri version drift:** CLI 2.5.0, crate 2.8.5, JS 2.10.x — MEDIUM
- **Three Cargo.lock files:** backend, tauri, core — LOW (intentional)
- **Catalog vs workspace styles:** dual dependency style — LOW (cosmetic)

## Core (from problem-summary.md)
- **Auth fundamentally broken:** PASETO vs Supabase JWT mismatch — CRITICAL
- **Instance data model incomplete:** missing game_type, version, port, etc. — HIGH
- **Supabase integration missing:** no heartbeat, no JWT validation — HIGH
- **Process kill doesn't work:** ghost servers keep running — HIGH
- **Friend groups tables missing:** only individual friendships exist — MEDIUM
- **Mod sync flow missing:** owner push → member download — MEDIUM

## Quality
- **No CI/CD:** missing GitHub Actions — HIGH (user will learn later)
- **No real tests:** only vue-tsc typecheck — MEDIUM (planning axum-test + invoke script)
- **Supabase credentials committed:** security risk — SECURITY (rotate immediately)

## Lodestone Legacy (from old bugs.md)
- Console ANSI colors stripped — MEDIUM
- No setup status check (re-runs first-time setup) — MEDIUM
- playit.gg API outdated — LOW
- UPnP stubbed out — LOW