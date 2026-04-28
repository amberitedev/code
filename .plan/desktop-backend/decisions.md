# Desktop Backend Decisions

Architectural decisions for `amberite-backend` and the Tauri plugin layer.
Last updated: 2026-04-27

---

## Auth: Microsoft → Supabase via Edge Function

**Decision:** Use a Supabase Edge Function (`microsoft-auth`) to exchange the
Microsoft ID token (obtained by Theseus's existing Xbox/MSAL auth flow) for a
Supabase JWT. User never touches a password or sees a second login prompt.

**Flow:**
1. User clicks "Sign in with Microsoft"
2. Theseus opens MSAL in a Tauri webview → user signs in → Theseus stores Xbox token
3. `amberite-backend` takes the Microsoft ID token from Theseus's credential store
4. Calls Supabase `microsoft-auth` Edge Function with the token
5. Edge Function verifies the token with Microsoft, creates or links a Supabase user
6. Returns a Supabase JWT → stored in OS keychain
7. All subsequent Core API calls include this JWT as `Authorization: Bearer <token>`

**Why:** Theseus already handles Microsoft/Xbox login for Minecraft. Reusing this
avoids a second login step. The Edge Function handles account creation/linking invisibly.

**Web-browser OAuth (Google, Discord, GitHub):** Web dashboard only. Not in the desktop
app at all. These providers give a "lesser" account with no Minecraft identity.

**Multi-account:** Single account for V1. Multi-account switcher is a V2 feature.

---

## Core Connection: Local + Remote via Pairing Code

**Decision:** Core can run locally (localhost) or on any remote machine. Connection
is established via a one-time 6-digit pairing code that Core prints to stdout on
first startup if it has no owner yet.

**Localhost flow (auto-pair):**
1. App spawns Core as a background child process
2. App reads Core's stdout, detects the pairing code
3. App automatically calls POST `/setup` with the code + Supabase user ID
4. User becomes owner — no manual steps

**Remote flow (manual):**
1. User runs Core on their NAS/VPS/server
2. Core prints pairing code to terminal
3. User opens "Connect to Server" in the app, enters the Core URL + pairing code
4. App calls POST `/setup` on the remote Core
5. User becomes owner

**Owner status is stored in Supabase and is permanent.** It cannot be changed.
Core URL is stored locally in `settings.json`, not in Supabase.

**For friends (non-owners):** Connect to a Core by being a member of a friend group.
No pairing code needed. Supabase routes group membership → Core URL discovery via
`core_registrations` table.

---

## Core Auto-Start: App Manages Core as a Background Process

**Decision:** The Desktop App is the "CLI" for Core on desktop. It downloads,
spawns, monitors, and restarts Core as a background child process.

**Core is an optional feature.** New users do not need Core. It is a feature they
install if they want to host a server. They can uninstall it independently.

**Core data is stored separately from the app.** Data lives in
`{AppData}/amberite-core/` and is NEVER deleted when the app is uninstalled.
This protects server worlds and configs.

**Startup behavior is configurable:**
- Start Core when Amberite launches
- Start Core when the PC starts (OS startup registration)
- Never auto-start (manual only)

---

## Library Page: Merged Client + Server with Filter Chips

**Decision:** The Library page shows ALL Minecraft instances — local client profiles
AND server profiles from connected Core — in one unified list. Filter chips at the
top: `All`, `Client`, `Server`.

**Rationale:** Amberite's entire premise is that server and client always share the
same mods. A unified library makes this relationship visible. Separating them into
two screens creates a false divide and hides the core product story.

**Server instance cards** get a status badge (Running / Offline / Starting) and a
quick-action row: Start, Stop, Console, Push Mods.

**Modrinth community servers** (public play servers from Modrinth) stay as a separate
tab. They are a fundamentally different concept (joining public servers, not managing
your own) and should not be mixed with personal instances.

---

## Tunneling: Playit.gg + Cloudflare DNS for Custom Subdomain (V2)

**Decision:** Use Playit.gg for TCP tunneling combined with Cloudflare DNS for the
`{servername}.amberite.dev` branded subdomain.

**Why not Cloudflare Tunnels:** Free tier is HTTP-only. Minecraft uses raw TCP.
Cloudflare Spectrum (TCP support) is enterprise pricing.

**Why Playit.gg:** Purpose-built for game server TCP tunneling. Free tier works.
Friends connect without any client setup.

**How the subdomain works:**
1. App bundles the Playit.gg agent binary
2. On tunnel setup, Playit.gg assigns a `{random}.ply.gg` address
3. App calls Cloudflare DNS API to create a CNAME:
   `{servername}.amberite.dev` → `{random}.ply.gg`
4. Friends connect to `{servername}.amberite.dev:25565` — no port forwarding needed

**V1 ships without tunnel.** Users do port forwarding manually in V1.
Tunnel feature is V2.

---

## Friend Groups: Per-Server, Discord-Like Permissions

**Decision:** A friend group is tied to a Core instance. Default join role is
view-only (can see the server, cannot control it). Permissions are granular
and Discord-style (presets + custom bitfield for fine control).

**Example permission presets:**
- Viewer: can see status, see console (read-only), download pack
- Member: viewer + can join/leave server
- Mod: member + can start/stop server, add mods (with vote?)
- Admin: mod + can restart, manage members, delete instances
- Owner: full control, cannot be removed

**Friends list** is flat (Steam-style). A `friends` table stores pairs alphabetically
sorted to prevent duplicate rows. Friends can be found by username or friend code
(both map to UID).

**Invite links** are 24h UUID tokens stored in `group_invites`. Anyone with the link
becomes a friend + joins the group at the default (viewer) role.

**Group invites and friend invites are separate:** joining a group automatically
creates the friendship, but adding a friend does not automatically add them to a group.

---

## Mod Sync: Hybrid Event + Periodic Full Snapshot

**Decision:** Mod sync uses a hybrid model:
- **Events:** Every modpack change emits a row insert to `mod_sync_events` via
  Supabase Realtime → clients receive push notifications immediately
- **Full snapshots:** Every ~10 events, the owner sends a complete `.mrpack` snapshot
  to ensure clients that were offline stay in sync

**Initial download:** When a friend first joins a group, they download the full current
`.mrpack` from Core. This is the baseline.

**Client preferences:** After downloading the server modpack, client-side-only mods
(shaders, OptiFine, etc.) are merged from the user's local preferences. The server
defines required mods; the client adds optional ones on top.

**Who triggers sync:**
- Owner (or admin with permission) clicks "Push to Core" in the app
- App exports their Theseus profile as `.mrpack`
- App POSTs to Core `POST /instances/:id/modpack`
- Core installs it and inserts a row into `mod_sync_events`
- Friends' apps receive the Realtime event, show a "Update available" badge

---

## Error Types: Single Unified `AmberiteError`

**Decision:** Remove the current dual-error inconsistency. There are currently two
error enums (`AmberiteError` in `lib.rs` and `BackendError` in `error.rs`). These
must be merged into a single `AmberiteError` in `error.rs` before adding new modules.

The existing `AmberiteCommandError` in `tauri/src/amberite.rs` stays separate —
it is the Tauri serialization wrapper, not a backend error type.

---

## `amberite-backend` Stays Isolated from Theseus

**Decision:** `amberite-backend` never modifies Theseus internals. Any Theseus
behavior that needs patching is handled by copying the relevant file into
`apps/app/backend/src/` and marking changed lines with `// AMBERITE PATCH`.

The copy and its patch locations are documented in `apps/app/AGENTS.md`.

This keeps the backend crate safe from upstream merge conflicts when Modrinth
updates Theseus.

---

## Onboarding Flow

**Decision:** First-run onboarding follows this linear path:

1. **Welcome screen** — App name, tagline, "Get Started" button
2. **Microsoft login** — In-app MSAL webview (same flow Theseus already has).
   After login, Supabase account is created silently via `microsoft-auth` Edge Function.
3. **Core setup** — Two options:
   - "Set up a server on this PC" → app downloads and starts Core locally, auto-pairs
   - "Connect to an existing server" → user enters Core URL + pastes pairing code
   - "Skip for now" → goes straight to main app (can set up later via Servers page)
4. **Main app** — Library page (empty state with guidance if no instances)

---

## Servers Page Replacement

**Decision:** Replace the Modrinth billing/paid-hosting UI in `Servers.vue` with
"My Servers" — a list of connected Core instances.

Each Core card shows:
- Server name + online/offline status
- Player count (if running)
- Active instance count
- Quick actions: Open Dashboard, Start All, Stop All

Modrinth community servers (public servers from Modrinth) stay as a second tab.
They are NOT removed — they serve as a useful reference and potential future use.
