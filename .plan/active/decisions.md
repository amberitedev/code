# Architectural Decisions

**Last updated:** 2026-04-19

---

## Mod Sync Approach (Clarified by User)

**NOT sending binaries.** Manifest-based sync using Modrinth API:

- Modrinth hosts all mods with versions — use their API to download
- Sync sends manifest (`.mrpack` format) listing what to download
- Only custom/private mods (not on Modrinth) need physical transfer
- `.mrpack` format: `modrinth.index.json` + ZIP of custom files
- Exporter/unexporter exists in Theseus — reuse it

**Reference:** Theseus (apps/app in modrinth/code), mrpack-install

---

## Reference Project Philosophy

Projects are **possible inspiration**, not concrete implementations:

- Look at how they solve a problem
- Evaluate: Does their approach match Amberite's constraints?
- If yes → consider adopting pattern
- If no → learn why, understand tradeoffs

**Don't scour web excessively.** Start with:
1. Modrinth's own code (you forked it)
2. Modrinth awesome list (https://github.com/modrinth/awesome)
3. Theseus documentation (https://docs.modrinth.com/contributing/theseus)

---

## Testing Approach
- **Core:** `axum-test` for endpoint testing
- **Frontend:** Tauri invoke test script (runs outside CI)
- **CI:** User will learn GitHub Actions later

---

## Dependency Strategy
- **Modrinth packages:** Single style (`workspace:*` only, remove catalog)
- **Fork:** Stay on latest Modrinth release, periodic merges
- **Reference preferred** over local copy modification

---

## Product Boundaries
- **Core:** Independent backend, standalone or launched by app
  - Windows + Linux
  - Port-forwarded when app launches local core
  - NAS, cloud, or local machine
- **Web:** Dashboard + marketing + docs (lower priority)
- **CLI:** Late-stage, Linux-only installer/runner/updater

---

## Companion Mod Approach

**Likely just use/port Essential Mod:**
- Essential mod has similar features (social, config sync)
- Automatically hosted when joining/creating world
- Port UI, swap backend to use Amberite Core
- OR just use Essential directly — undecided

**Reference:** Essential Mod (https://github.com/SparkUniverse/Essential-Mod)

---

## Century (Log/Crash AI)

**Possible references:**
- Sentry Seer AI — LLM for root cause analysis
- Graylog pipelines — rule-based parsing
- mcla — Minecraft-specific crash patterns
- Drain3 — template extraction

**Decision pending:** Pattern DB + LLM vs pure ML vs hybrid

---

## Friend Groups

**Possible references:**
- Mattermost (Teams + Channels) — shows Go backend pattern
- Spacebar (Discord clone) — shows relationships/permissions schema
- Keycloak (Realms) — shows tenant isolation

**Simpler approach:** Owner/Admin/Member/Guest (GitLab 4-tier)

---

## P2P Failover

**Possible references:**
- Consul sessions — lock tied to health check
- Redis Redlock — quorum-based election

**Hard problems:** World conflict (branch vs disposable), quorum size

---

## Supabase Auth
- Wait for Supabase MCP from user
- Current: Anon key (risky) — fix later

---

## Memory System
- **feature-memory skill:** Tracks session, writes on request/end
- **Files:** `.plan/active/` (current), `.plan/archive/` (history), `.plan/completed/` (done)
- **Agent prompts:** Baked into plan/build system prompts