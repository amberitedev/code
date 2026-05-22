# Amberite Core — Sync & Mod Management Plan

This document describes the complete system design for making the Core's mod-management and profile-sync workflows fully functional. It is written in plain English — no code — so the design can be reviewed before implementation begins.

---

## 1. Philosophy & Constraints

- The Core is a **stateful server**. It owns the instance filesystem, the mod list, and the authoritative version history.
- The desktop app (the client) is a **thin controller**. It initiates actions, but the Core executes them and reports back.
- **Mrpacks are the sync currency.** A `.mrpack` ZIP is the exact snapshot format. It contains the index, the mod JARs, config overrides, and datapacks. The Core stores these archives as version artifacts.
- **Versions are identified by UUIDs, not semver.** Every sync push creates a new snapshot UUID. Clients simply ask "what is the current UUID for profile X?" and compare.
- **Forward-only for now.** The Core stores every snapshot, but the "active" version is always the latest successful one. Rollback is deferred.
- **Mods only for now.** Configs, datapacks, and resource packs live inside the mrpack and are extracted, but the diff algorithm only cares about mod JARs for the first iteration.

---

## 2. Current State Assessment

The Core already has the following working pieces:

| Feature | Status | Notes |
|---------|--------|-------|
| Instance CRUD | Complete | Create, start, stop, delete, properties, stats |
| Mod install (single) | Complete | `POST /instances/:id/mods` resolves dependencies via internal Modrinth client |
| Mod upload/delete/toggle | Complete | File ops + DB tracking |
| Mod update (single + all) | Complete | Checks Modrinth for newer version, downloads, swaps |
| Mrpack install | Complete | `install_mrpack` extracts metadata, downloads server-side files, extracts overrides |
| Mrpack export | Complete | `export_modpack` builds `.mrpack` from tracked mods + config overrides |
| Sync profile scaffolding | Partial | Tables `sync_profiles`, `sync_snapshots`, `sync_events` exist. APIs register profiles and store raw JSON manifests. No diff/apply logic. |
| Event stream (SSE) | Complete | `/events` emits instance lifecycle changes |
| Relay messaging | Complete | `/relay/messages` stores durable messages for client polling |
| Internal Modrinth client | Complete | Used for dependency resolution during mod install. Not exposed as HTTP routes. |

**What is missing:**
1. A complete, polished mod-list API that returns everything a client needs (the existing `GET /instances/:id/mods` is close but the Core should treat this as a first-class contract).
2. The ability to create a **new sync profile from an mrpack** (not just update an existing one).
3. The **diff/apply algorithm** that turns a published mrpack snapshot into actual filesystem changes on an instance.
4. **Version promotion:** marking a snapshot as "active" and publishing that fact to clients.
5. A **version-check API** so clients can poll "is my profile up to date?"
6. **Sync event persistence** with delivery guarantees — clients must not miss "profile updated" events.

---

## 3. System 1: Mod Management API (Polished)

The Core already has all the pieces. This system is about making the API contract explicit, stable, and complete.

### 3.1 Data Contract

The Core exposes a mod as:

- `id`: Core-generated UUID (or `null` for untracked JARs dropped into `mods/`)
- `filename`: The JAR filename on disk
- `display_name`, `version_number`: Human-readable metadata from Modrinth
- `enabled`: Whether the JAR is active (vs `.jar.disabled`)
- `tracked`: Whether the Core has a DB row for this mod
- `client_side`, `server_side`: Environment compatibility from Modrinth project
- `modrinth_project_id`, `modrinth_version_id`: Linkage for updates
- `update_available`: `true` if a newer Modrinth version exists, `false` if latest, `null` if unknown
- `sha512`: Hash for integrity verification
- `installed_at`: RFC 3339 timestamp

### 3.2 Endpoints

| Method | Route | Purpose |
|--------|-------|---------|
| GET | `/instances/:id/mods` | List all mods (tracked + untracked) with full metadata |
| POST | `/instances/:id/mods` | Install a mod from Modrinth by `version_id` or `project_id`. Core resolves the latest compatible version and **all required dependencies**. |
| POST | `/instances/:id/mods/upload` | Upload a JAR directly. Tracked but without Modrinth linkage. |
| DELETE | `/instances/:id/mods/:filename` | Delete a mod (and its DB record). |
| PATCH | `/instances/:id/mods/:filename` | Toggle enabled/disabled by renaming `.jar` <-> `.jar.disabled`. |
| PUT | `/instances/:id/mods/:filename/update` | Update a single tracked mod to its latest compatible Modrinth version. |
| POST | `/instances/:id/mods/update-all` | Update all tracked mods in one batch. Returns `{ updated, already_latest, failed }`. |

### 3.3 Dependency Resolution

When installing by `project_id` or `version_id`, the Core:

1. Fetches the target version from Modrinth.
2. Checks the project's `server_side` field — rejects if `unsupported`.
3. Uses a stack-based DFS to resolve `required` dependencies:
   - If dependency has a `version_id` and that version is not already installed, fetch it and queue.
   - If dependency only has a `project_id`, look up the latest compatible version (matching instance game_version + loader) and queue.
   - Skip any dependency already installed (checked by `modrinth_version_id` or `modrinth_project_id` in DB).
4. Downloads each queued mod, verifies SHA512, writes to `mods/`, inserts DB record.

The Core **keeps its internal Modrinth client** for this. It does not expose Modrinth search/browse endpoints — those live in the desktop app or website, which use their own Modrinth API client.

---

## 4. System 2: Sync Profile Lifecycle

A **Sync Profile** is a named, versioned collection of mods (and later configs) that is shared between the Core and one or more clients. It is **not** the same as an Instance. A profile can be mapped to an instance (the Core's copy), but the profile itself is the abstract mod list.

### 4.1 Profile Creation

There are two ways to create a profile:

**A. Create from scratch (client-driven)**
- Client calls `POST /sync/profiles` with `{ name, game_version, loader }`.
- Core creates an empty `sync_profiles` row.
- Client later pushes mrpack snapshots to this profile.

**B. Create from an existing instance or mrpack (core-driven)**
- Client uploads a `.mrpack` file to `POST /sync/profiles/from-mrpack`.
- Core:
  1. Unpacks the mrpack into a **temporary staging directory** (not directly into an instance).
  2. Reads `modrinth.index.json` to build the mod manifest.
  3. Creates a new `sync_profiles` row.
  4. Creates the first `sync_snapshots` row pointing to the mrpack archive.
  5. Creates the first `sync_events` row with status `completed`.
  6. Optionally, if `core_instance_id` is provided, applies the snapshot to that instance immediately.

### 4.2 Profile Structure

A profile has:
- `id`: UUID (primary key)
- `name`: Human-readable label
- `game_version`, `loader`: Constraints — all snapshots must match these
- `sync_enabled`: Whether auto-sync is active
- `current_snapshot_id`: The UUID of the latest successfully applied snapshot
- `created_at`, `updated_at`, `last_snapshot_at`

### 4.3 Profile ↔ Instance Mapping

A profile is optionally linked to a Core instance via `sync_profiles.core_instance_id`. This is the instance that receives the applied mods. Multiple profiles can exist, but only one should be "active" per instance at a time.

---

## 5. System 3: Snapshot Diff / Apply Algorithm

This is the heart of the sync system. When a client pushes a new snapshot, the Core must reconcile it with the current state of the mapped instance.

### 5.1 Push Flow

1. **Client uploads mrpack.**
   - `POST /sync/profiles/:id/snapshots` with a multipart upload containing the `.mrpack` file.
   - The body also includes optional metadata: `author_user_id`, `notes`, `server_manifest_json`.

2. **Core stores the archive.**
   - The raw `.mrpack` bytes are saved to `{data_dir}/sync_archives/{snapshot_id}.mrpack`.
   - A `sync_snapshots` row is inserted with the manifest JSON extracted from `modrinth.index.json`.
   - A `sync_events` row is created with status `applying`.

3. **Core runs the diff algorithm.**
   - Read the current snapshot's mod list from the mrpack index.
   - Read the current instance's mod list from `mods/` + DB.
   - Compute the delta:
     - **Additions**: Mods in snapshot but not in instance.
     - **Removals**: Mods in instance but not in snapshot.
     - **Updates**: Mods where `modrinth_version_id` differs (or SHA512 differs for untracked mods).
     - **No-ops**: Mods already matching exactly.
   - Store the diff JSON in `sync_events.diff_json`.

4. **Core applies the diff.**
   - For additions: download the mod from the URL in the mrpack index, verify SHA1, write to `mods/`, insert DB record.
   - For removals: delete the JAR, delete the DB record.
   - For updates: download the new version, swap the JAR, update the DB record.
   - For mrpack entries that are not Modrinth-hosted (no download URL, but present as `overrides/mods/*.jar` in the ZIP): extract the JAR directly from the mrpack archive and copy it to `mods/`.
   - For overrides (configs, datapacks): extract from mrpack's `overrides/` and `server-overrides/` into the instance directory.

5. **Core marks snapshot active.**
   - Update `sync_profiles.current_snapshot_id` to this snapshot's UUID.
   - Update `sync_events.status` to `completed`.
   - Update `sync_profiles.updated_at`.

6. **Core broadcasts the sync event.**
   - Emit a `SyncProfileUpdated` event on the SSE stream.
   - Write a relay message for any offline clients to pick up later.

### 5.2 Diff Data Structure

The diff JSON stored in the event:

```json
{
  "added": [{ "path": "mods/foo.jar", "hashes": { "sha1": "..." }, "downloads": ["..."] }],
  "removed": [{ "filename": "mods/bar.jar", "modrinth_version_id": "..." }],
  "updated": [{ "filename": "mods/baz.jar", "old_version_id": "...", "new_version_id": "..." }],
  "unchanged": ["mods/qux.jar"],
  "overrides_extracted": ["config/thing.cfg"]
}
```

### 5.3 Error Handling During Apply

If any step fails (download error, hash mismatch, disk full):
- `sync_events.status` becomes `failed`.
- `sync_events.message` contains the error.
- The instance is left in a **best-effort** state — successfully applied changes are not rolled back.
- The client receives a 422 or 500 response with details.
- The profile's `current_snapshot_id` is **not** updated.

### 5.4 Server-Side Only

The mrpack index contains `env.client` and `env.server` fields. During apply, the Core:
- **Skips** files where `env.server == "unsupported"`.
- **Includes** files where `env.server` is `"required"` or `"optional"` or absent.
- Extracts all `overrides/` and `server-overrides/` entries regardless of env.

---

## 6. System 4: Version Publishing & Client Notification

When a snapshot is successfully applied, all connected and future clients need to know.

### 6.1 SSE Event

The Core emits a new `Event` variant:

```
SyncProfileUpdated {
  profile_id: String,
  snapshot_id: String,
  instance_id: Option<String>,
}
```

This is broadcast on the existing `/events` SSE stream. Any connected client sees it immediately.

### 6.2 Persistent Relay Message

For clients that are offline or reconnect later, the Core writes a relay message:

- `type: "sync_profile_updated"`
- `recipient_id`: `"*"` (broadcast to all group members) or the specific group ID
- `payload`: `{ profile_id, snapshot_id, instance_id }`
- `ack`: `"received"` (clients must ack to mark it seen)

Clients poll `GET /relay/messages/:recipient_id` on reconnect and process pending sync notifications.

### 6.3 Version Check API

Clients should not rely solely on events. They also poll:

`GET /sync/profiles/:id/check-version`

Response:
```json
{
  "profile_id": "...",
  "current_snapshot_id": "...",
  "current_snapshot_created_at": "..."
}
```

The client compares `current_snapshot_id` to its locally stored one. If different, it knows it needs to pull the new state (either by re-downloading the mrpack or by trusting the Core to have applied it already).

---

## 7. System 5: Snapshot Storage & Retention

All `.mrpack` archives are stored on disk in a configurable directory (default: `{AMBERITE_DATA_DIR}/sync_archives/`).

### 7.1 Retention Policy

A new config variable `AMBERITE_SYNC_RETAIN_COUNT` (default: 10) controls how many archives per profile are kept.

When a new snapshot is created:
1. Save the new archive.
2. Query the 11th-oldest snapshot for this profile.
3. Delete its archive file from disk.
4. Keep the `sync_snapshots` DB row for history, but mark it `archived = true`.

This means the DB always has the full history, but disk space is bounded.

### 7.2 Re-downloading Old Snapshots

If a client needs an old snapshot whose archive was GC'd, it gets a 410 response. The client must then reconstruct its local state from the current snapshot instead. (This is acceptable because sync is forward-only.)

---

## 8. Data Model Additions

New columns and tables needed:

### `sync_profiles` additions
- `current_snapshot_id TEXT` — the active snapshot UUID

### `sync_snapshots` additions
- `archive_path TEXT` — path to the stored `.mrpack` on disk
- `archived INTEGER DEFAULT 0` — whether the disk file was GC'd

### `sync_events` enhancements
- `status` values: `planned`, `applying`, `completed`, `failed`, `rolled_back`
- `diff_json TEXT` — the computed diff
- `applied_at TEXT` — when the apply finished

### New event types in `domain/event.rs`
- `SyncProfileUpdated { profile_id, snapshot_id, instance_id }`
- `SyncEventStatusChanged { profile_id, event_id, status, message }`

---

## 9. API Route Summary

### Existing routes (already implemented, keep as-is)

| Method | Route | Purpose |
|--------|-------|---------|
| GET | `/instances/:id/mods` | List mods |
| POST | `/instances/:id/mods` | Install mod with deps |
| POST | `/instances/:id/mods/upload` | Upload JAR |
| DELETE | `/instances/:id/mods/:filename` | Delete mod |
| PATCH | `/instances/:id/mods/:filename` | Toggle mod |
| PUT | `/instances/:id/mods/:filename/update` | Update mod |
| POST | `/instances/:id/mods/update-all` | Update all mods |
| POST | `/instances/:id/modpack` | Install mrpack to instance |
| POST | `/instances/:id/modpack/modrinth` | Install Modrinth mrpack version |
| GET | `/instances/:id/modpack` | Get modpack manifest |
| DELETE | `/instances/:id/modpack` | Remove modpack |
| GET | `/instances/:id/modpack/export` | Export instance as mrpack |

### Modified / enhanced routes

| Method | Route | Change |
|--------|-------|--------|
| POST | `/sync/profiles` | Keep existing — creates empty profile |
| POST | `/sync/profiles/:id/snapshots` | **Change from JSON manifest to multipart mrpack upload.** This is the main push endpoint. |
| GET | `/sync/profiles/:id/snapshots` | Keep existing — list history |
| GET | `/sync/profiles/:id/events` | Keep existing — list sync events |

### New routes

| Method | Route | Purpose |
|--------|-------|---------|
| POST | `/sync/profiles/from-mrpack` | Create a brand new profile from a uploaded `.mrpack`. |
| GET | `/sync/profiles/:id/check-version` | Return the current active snapshot UUID for this profile. |
| GET | `/sync/profiles/:id/snapshots/:sid/download` | Download the raw `.mrpack` archive for a snapshot. |
| POST | `/sync/profiles/:id/apply/:sid` | Re-apply a specific historical snapshot to the linked instance. (Deferred to post-MVP.) |

---

## 10. Event Stream Additions

The SSE `/events` stream gains two new event types:

1. **`sync_profile_updated`**
   ```json
   {
     "type": "sync_profile_updated",
     "profile_id": "uuid",
     "snapshot_id": "uuid",
     "instance_id": "uuid-or-null"
   }
   ```
   Emitted when a snapshot is successfully applied and the profile's current version changes.

2. **`sync_event_status_changed`**
   ```json
   {
     "type": "sync_event_status_changed",
     "profile_id": "uuid",
     "event_id": "uuid",
     "status": "applying|completed|failed",
     "message": "..."
   }
   ```
   Emitted when a sync event transitions state (e.g., apply starts, succeeds, or fails).

---

## 11. Complete User Flows

### Flow A: Create a new sync profile from an mrpack

1. User has a `.mrpack` file (exported from their desktop app or downloaded from Modrinth).
2. Client `POST /sync/profiles/from-mrpack` with the file multipart upload.
3. Core saves the archive, creates the profile, creates the first snapshot, unpacks the mrpack into a staging area, reads the index, optionally applies to an instance.
4. Core responds with `{ profile, snapshot, event }`.
5. If applied successfully, the profile's `current_snapshot_id` is set. Other clients see `sync_profile_updated` on SSE.

### Flow B: Push a new snapshot to an existing profile

1. User adds/removes/updates mods in their desktop app profile.
2. Desktop app rebuilds the `.mrpack` ZIP locally.
3. Client `POST /sync/profiles/:id/snapshots` with the new mrpack.
4. Core stores the archive, creates a snapshot row, creates an event row with status `applying`.
5. Core runs diff against the linked instance's current mods.
6. Core applies additions/removals/updates.
7. Core updates profile `current_snapshot_id = new_snapshot_id`.
8. Core marks event `completed`.
9. Core emits `sync_profile_updated` SSE event + writes relay message.
10. All connected clients see the event and update their local state.

### Flow C: Client reconnects and checks version

1. Client starts up and connects to Core SSE.
2. Client calls `GET /sync/profiles/:id/check-version`.
3. Core responds with `current_snapshot_id`.
4. Client compares to its local `last_known_snapshot_id`.
5. If different, client either:
   - a) Pulls the mrpack via `GET /sync/profiles/:id/snapshots/:sid/download` and re-imports locally, OR
   - b) Trusts that the Core already applied it to the server and simply updates its local metadata.

### Flow D: Install a single mod with dependencies

1. User selects a mod in the desktop app (browsing via their own Modrinth API client).
2. Client calls `POST /instances/:id/mods` with `{ version_id: "..." }`.
3. Core fetches the version from Modrinth, resolves required deps, downloads all missing ones, verifies hashes, installs to `mods/`.
4. Core responds with the root mod's `ModInfo`.
5. Core emits no sync event (this is an instance-level change, not a profile sync).

---

## 12. Configuration Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `AMBERITE_SYNC_ARCHIVES_DIR` | `{AMBERITE_DATA_DIR}/sync_archives` | Where `.mrpack` snapshots are stored on disk |
| `AMBERITE_SYNC_RETAIN_COUNT` | `10` | Max archives per profile kept on disk |

---

## 13. Security Notes

- **Mrpack download URLs:** The existing `validate_download_url` in `mrpack.rs` restricts downloads to `cdn.modrinth.com`, `github.com`, and `*.githubusercontent.com`. This stays.
- **Path traversal:** The existing `guarded_child_path` stays. All mrpack extraction paths are validated.
- **Archive size limits:** A new multipart limit should be applied on mrpack upload (e.g., 500MB).
- **Auth:** All sync routes require `AuthUser` (Bearer JWT or dev bypass).

---

## 14. Deferred to Future Iterations

These are explicitly out of scope for the first implementation:

- **Config sync beyond overrides:** The mrpack `overrides/` are extracted, but the diff algorithm does not detect individual config file changes. It treats the entire mrpack as a opaque snapshot.
- **Rollback / checkout:** Snapshots are stored, but there is no API to revert an instance to an older snapshot.
- **Multi-instance profiles:** One profile maps to one instance for now.
- **Client-side mod enforcement:** The Core tells clients the new snapshot UUID, but it does not force the client to install specific client-side mods. The client pulls the mrpack and decides.
- **Concurrent sync locks:** If two clients push simultaneously, the second should wait or be rejected. A simple `sync_events.status = 'applying'` check is sufficient for MVP.

---

## 15. What to Build (Checklist)

1. **Migration:** Add `current_snapshot_id` to `sync_profiles`, `archive_path` and `archived` to `sync_snapshots`, `applied_at` to `sync_events`.
2. **Event types:** Add `SyncProfileUpdated` and `SyncEventStatusChanged` to `domain/event.rs`. Wire them into `events.rs` SSE handler.
3. **New handler `sync_create_from_mrpack`:** Accept multipart mrpack, create profile + snapshot, store archive.
4. **Rewrite `publish_snapshot`:** Change from JSON manifest to multipart mrpack upload. Implement the full diff/apply pipeline.
5. **Diff service:** A new `application/sync_diff_service.rs` that computes the delta between two mod lists.
6. **Apply service:** A new `application/sync_apply_service.rs` that downloads/removes/swaps JARs based on the diff.
7. **Archive storage service:** A new `application/sync_archive_service.rs` that saves/retrieves/GCs `.mrpack` files on disk.
8. **Version check endpoint:** `GET /sync/profiles/:id/check-version`.
9. **Relay integration:** Write a relay message on successful snapshot apply.
10. **amberite-api types:** Add TypeScript types for new endpoints and events to `packages/amberite-api/src/types.ts` and `api.ts`.
