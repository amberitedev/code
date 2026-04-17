# Modstone — Problem Summary

> **Status:** Initial analysis complete. This file documents all identified problems with the current setup.

---

## **1. AUTHENTICATION FUNDAMENTALLY BROKEN**

**The entire auth architecture contradicts PROJECT.md:**

| What PROJECT.md Says | What Core Actually Does |
|---------------------|------------------------|
| Supabase handles all login | Core has its own `users` table with passwords |
| Supabase issues JWTs | Core uses PASETO tokens |
| Core never stores passwords | Core hashes passwords with Argon2 |
| Core validates JWT signature mathematically | Core generates its own tokens via `/login` |

**Sub-problems:**
- `/setup` creates a local admin user, not a one-time owner key tied to Supabase user
- No Supabase JWT validation middleware on protected routes
- No owner claim flow (enter key → Core links to your Supabase account)
- No member key generation for inviting others with scoped permissions
- WebSocket auth via URL query parameter (security leak)
- `/setup` can be called multiple times (no first-run guard)
- PASETO key generation/rotation undefined
- `cores.core_secret_hash` empty/unused for heartbeat verification

---

## **2. INSTANCE DATA MODEL COMPLETELY MISSING**

**Core stores only `id` + `name` + `created_at`. Everything else is missing:**

| Panel Expects | Core Has |
|--------------|----------|
| `game_type` (vanilla, paper, fabric, etc.) | ❌ |
| `version` (Minecraft version) | ❌ |
| `java_version` | ❌ |
| `port` | ❌ |
| `path` (instance directory) | ❌ |
| `status` (starting/stopping/crashed states) | ❌ (only Stopped/Running) |
| `modpack_config` (linked Modrinth project) | ❌ |
| `uuid` (what is this?) | ❌ |

**Sub-problems:**
- No instance directory structure convention
- Process spawn command hardcoded (`java -jar server.jar`) — can't vary by instance
- No port allocation strategy for multiple instances
- No `POST /instances` endpoint — Panel expects to create instances
- Server properties not stored/persisted per instance
- Console events not saved to `events` table (lost on restart)
- Intermediate states missing: starting, stopping, crashed
- No crash detection (process alive ≠ server healthy)

---

## **3. SUPABASE INTEGRATION NON-EXISTENT**

**Core should sync with Supabase but doesn't:**

| Should Happen | Actually Happens |
|--------------|------------------|
| Core sends heartbeat to Supabase on startup | ❌ Nothing |
| Core reports `is_online`, `player_count`, `game_version` | ❌ Fields exist but unused |
| Core validates incoming Supabase JWTs | ❌ Uses own PASETO |
| Supabase tracks which modpacks are on which Core | ❌ No table for this |

**Sub-problems:**
- `core-heartbeat` Edge Function exists but Core doesn't call it
- `cores.core_secret_hash` meant for verification but empty
- `cores.connection_code` unused (no flow for members to connect)
- Supabase `users.id` should equal auth users, but Core has separate UUIDs
- No offline/local dev mode when Supabase unreachable

---

## **4. FRIEND GROUPS vs FRIENDSHIPS CONCEPT MISSING**

**PROJECT.md describes friend groups tied to Cores. Supabase only has individual friendships.**

| PROJECT.md Concept | Supabase Has |
|-------------------|--------------|
| Friend Group = people on same Core | ❌ Only `friendships` (user pairs) |
| Group owner permissions (roles) | ❌ |
| Group invite links | ❌ |
| Members see modpacks from their groups | ❌ |

**Missing tables:**
- `friend_groups` — id, core_id, owner_id, name
- `friend_group_members` — group_id, user_id, role/permissions
- `friend_group_invites` — for inviting to group
- `core_modpacks` — which modpacks on which Core

---

## **5. MOD SYNC FLOW COMPLETELY MISSING**

**This is the core feature per PROJECT.md. Nothing exists for it:**

| Owner Side | Member Side | Implementation |
|-----------|-------------|----------------|
| Push modpack to Core | See modpack in library | ❌ No flow |
| Core installs server version | Download client version | ❌ No version resolution |
| Server becomes joinable | Auto-connect (no address entry) | ❌ No address injection |

**Missing:**
- Core → Supabase: "I'm running modpack X version Y"
- Supabase → Member app: "Your friend's Core has modpack X"
- Member app → Minecraft: inject server address
- Client-side vs server-side modpack version resolution
- Personal preferences system (apply preferred mods on join)

---

## **6. INSTANCE MANAGEMENT CAPABILITIES MISSING**

**Core manages Minecraft servers as subprocesses but lacks basic management features:**

| Expected Feature | Status |
|-----------------|--------|
| Create instance (download server.jar, set up directory) | ❌ |
| Download from Vanilla/Paper/Fabric APIs | ❌ (flavours.rs has URLs but no download logic) |
| Install mods/modpacks | ❌ |
| File management (view/edit/upload files) | ❌ |
| World listing/backups | ❌ |
| Log persistence | ❌ (only live streaming) |
| Configuration editing (server.properties) | ❌ |
| Concurrent instance limit/resource guards | ❌ |
| Java validation (installed? correct version?) | ❌ |

---

## **7. NAMING/CONCEPTUAL CONFUSION**

| Issue | Details |
|-------|---------|
| Two "Instance" concepts | Panel `Instance` = server managed by Core; Panel `GameInstance` = local playable modpack. Routes `/instance/:id` vs Core `/instances/:id` |
| Three backends mixed | Panel talks to Modrinth API, Core API, and Supabase without clear separation |
| `uuid` field mystery | Panel Instance has `uuid` field — unclear what this represents vs `id` |

---

## **8. OPERATIONAL/QUALITY GAPS**

| Issue | Details |
|-------|---------|
| No API versioning | Breaking changes affect all clients |
| No rate limiting | Spam start/stop/kill, brute-force auth |
| No audit trail | Who did what to which instance when? |
| Invite/request expiration | Friend requests and invites never expire |
| WebSocket history | Reconnect loses all past console output |
| Timezone inconsistency | SQLite datetime vs Supabase timestamptz vs JS Date |
| No tests | Only `true == true` test exists |

---

## **PRIORITIZED FIX ORDER (based on PROJECT.md priorities)**

| Priority | Problem Group | Why First |
|----------|---------------|-----------|
| **1** | Authentication | Most urgent per PROJECT.md — security hole |
| **2** | Instance Data Model | Needed before anything else works |
| **3** | Supabase Integration | Core can't function without it |
| **4** | Process Kill Fix | Servers keep running as ghosts |
| **5** | Friend Groups Tables | Social features foundation |
| **6** | Mod Sync Flow | Core feature of entire project |
| **7** | Instance Creation/Management | Basic functionality |

---

## **TWO SPECIFIC CHANGES REQUESTED BY USER**

### **1. Core-to-Supabase Key System (Hashed Key Pattern)**

**Goal:** Core proves identity to Supabase without sending secrets over the wire.

**Proposed Flow (based on industry standards like GitHub PATs):**
1. Core generates a random key on first run
2. Core sends key to Supabase Edge Function `core-register`
3. Supabase stores **hash** of key (like password), returns `core_id`
4. Core saves original key in its local `paseto_key` table
5. Every heartbeat: Core sends original key → Supabase hashes and compares
6. If DB leaks: attackers get useless hashes, can't impersonate Core

**Question:** Should this key be used for:
- Core heartbeat/authentication only?
- OR also for user-to-Core authentication (users pasting code to connect)?

### **2. Simplified Login Flow**

**Goal:** Make connection as easy as possible.

**Proposed Flow (based on Discord/Steam invite patterns):**
1. User signs into Modstone account (OAuth: Modrinth/Google/GitHub)
2. User pastes connection code OR accepts friend group invite
3. System assigns default permissions (view-only, maybe start)
4. User can now access that Core/Server

**Question:** Is the "connection code" generated by:
- Owner's Core?
- Owner's Panel dashboard?
- Supabase when creating friend group?

**Default Permissions Question:**
- Per friend group (owner sets default for that group)?
- Per Core (owner sets global defaults)?
- Per user (owner assigns individually)?

---

## **INDUSTRY REFERENCE STANDARDS**

| Feature | Industry Standard | Why This Approach |
|---------|-------------------|-------------------|
| **API Keys** | GitHub Personal Access Tokens | Hashed storage, scoped permissions, revocable |
| **Friend Invites** | Discord server invites | Time-limited, role-based, one-click join |
| **Server Access** | Steam friend groups | Group tied to one server, permissions per role |
| **Heartbeat** | Minecraft server list pingers | Simple hash verification, no JWT overhead |
| **Invite Links** | Discord/Steam profile links | Short visible code, no auth required to join |

---

*Last updated: 2026-04-14*
