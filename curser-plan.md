---
name: core feature implementation roadmap
overview: Prioritized plan for Amberite Core focused on Modrinth-driven mod installation, mrpack pack/unpack, server-side install flow, and supporting foundation work from PROJECT.md so desktop and core can progress in parallel.
todos:
  - id: core-contracts
    content: Define install action contract, job states, and SQLite schema for jobs/events/source packs
    status: pending
  - id: modrinth-infra
    content: Implement Modrinth client and server-side compatibility filtering logic
    status: pending
  - id: mrpack-io
    content: Implement mrpack unpacker and packer with original pack retention
    status: pending
  - id: install-orchestrator
    content: Build per-instance install queue and stop-install-start orchestration
    status: pending
  - id: desktop-bridge
    content: Add desktop packet-to-core install mapping and progress/status integration
    status: pending
  - id: hardening
    content: Complete auth enforcement, process stop/kill reliability, and tests/docs
    status: pending
isProject: false
---

# Amberite Core Feature Plan (Mod Sync First)

## Context Anchors

- Project vision and constraints come from [c:\Users\ilai\amberite\PROJECT.md](c:\Users\ilai\amberite\PROJECT.md).
- Existing feature audit comes from [c:\Users\ilai\amberiteplan\features.md](c:\Users\ilai\amberiteplan\features.md).
- Core architecture boundaries come from [c:\Users\ilai\amberite\apps\core\AGENTS.md](c:\Users\ilai\amberite\apps\core\AGENTS.md) and [c:\Users\ilai\amberite\apps\core\src\AGENTS.md](c:\Users\ilai\amberite\apps\core\src\AGENTS.md).

## What To Build First (Best Features)

- **1) Core install action pipeline (highest value):** Add one install API that can accept desktop-triggered install actions (`install_mod`, `install_modpack`, `install_version`) and execute them asynchronously.
- **2) Modrinth integration service:** Query project/version metadata, infer side support (client/server/both), resolve files, and download verified artifacts.
- **3) `.mrpack` unpacker + packer:** Unpack modpack archives into instance workspace and also generate `.mrpack` exports for sharing/recovery.
- **4) Server-only mod filtering:** During install, include only server-compatible files and reject client-only mods for core-managed servers.
- **5) Original `.mrpack` retention:** Persist original uploaded pack per instance/version so friends can re-download the canonical pack.
- **6) Version/loader compatibility checks:** Validate Minecraft version and Fabric/Forge/NeoForge compatibility before writing files.
- **7) Config copy/merge flow:** Copy relevant override/config files from pack payload into server instance safely.
- **8) Install progress/events stream:** Expose status events (`queued`, `running`, `done`, `failed`) for desktop UX and troubleshooting.

## Foundation Work You Should Include (from PROJECT.md)

- **Auth hardening before broad rollout:** Protect install endpoints with real JWT/PASETO verification so only authorized users can trigger installs.
- **Reliable stop/kill behavior:** Fix process stop/kill path before automated install/restart flows to avoid ghost processes.
- **Persistent event/history storage:** Store install events, errors, and artifact metadata in SQLite for debugging and retries.
- **Minimal test baseline:** Add domain + service tests for compatibility filtering, pack parsing, and install orchestration.

## Lightweight Implementation Plan (Beginner-Friendly)

### Phase 0 — Contracts and Data Model

- Define install action request schema and response IDs.
- Add DB tables for install jobs, artifacts, source packs, and install events.
- Introduce a small state machine: `Queued -> Resolving -> Downloading -> Applying -> Completed/Failed`.

### Phase 1 — Modrinth + Compatibility Core

- Implement Modrinth API client module (project/version/file endpoints).
- Parse metadata to classify side support (`server`, `client`, `both`).
- Add compatibility checks for Minecraft version + loader (Fabric/Forge/NeoForge).

### Phase 2 — `.mrpack` Unpacker/Packer

- Unpacker: read `modrinth.index.json`, download listed files, apply overrides/configs.
- Packer: generate `modrinth.index.json` and zip export from instance content.
- Store original uploaded `.mrpack` binary and metadata for later friend downloads.

### Phase 3 — Server Install Orchestrator

- Build application-layer install orchestrator service using queue semantics per instance.
- Enforce stop-install-start flow (or block install if server is running until policy is chosen).
- Add rollback strategy (at least best-effort cleanup on failure).

### Phase 4 — Desktop/Core Integration

- Add endpoint for desktop "relevant packet" mapping to install actions.
- Add idempotency key (`action_id`) to prevent duplicate installs.
- Add status polling/stream endpoint for desktop progress UI.

### Phase 5 — Hardening and DX

- Add structured logs and event persistence.
- Add integration tests for one full `.mrpack` install scenario.
- Add docs: how install packets map to server behavior and how exports are retrieved.

## Suggested Feature Backlog (So You Don't Get Lost)

- **Install API v1** (single entrypoint + status endpoints)
- **Modrinth metadata resolver**
- **Server-side mod filter engine**
- **Pack storage service** (original `.mrpack` archive)
- **Pack export service** (packer)
- **Compatibility validator** (MC + loader matrix)
- **Config transfer strategy** (copy/merge/overwrite rules)
- **Install event bus + persistence**
- **Retry/rollback policies**
- **Auth middleware completion**
- **Process lifecycle fixes** (stop/kill correctness)
- **Friend pack download endpoint** (serve canonical original pack)

## Practical Defaults (Good For Your Experience Level)

- Start Modrinth-only (no CurseForge yet).
- Support Fabric first, then Forge/NeoForge.
- Use strict mode: block install on unknown compatibility.
- One install job per instance at a time.
- Keep rollback simple initially: backup replaced files + restore on failure.

## Parallel Work Split (Core vs Desktop)

- **Core:** install contracts, queue/orchestrator, modrinth resolver, pack unpack/pack, event/status API.
- **Desktop app:** map command packets to install contract, show install progress, surface compatibility errors, trigger pack download links.

## Research Notes To Ground Implementation

- Modrinth API docs and pack format spec should be treated as source of truth for metadata and `.mrpack` internals.
- Existing open-source mrpack tools can guide edge cases, but keep your own core interfaces clean and testable.
- Minecraft modding compatibility is mostly a matrix problem (MC version + loader + side support); model this explicitly in core domain types.