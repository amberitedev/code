# Amberite plan

The one plan document. It records goals and decisions, not implementation steps. When Ilai decides
something, it goes here in his terms, and whatever it replaces is removed. Open questions stay
listed as open; do not treat them as decided. Dev commands live in `scripts.md`.

Older planning files were merged into this one on 2026-10-10. They are in git history.

## The pieces

- **App** (`apps/app-frontend`, `apps/app`, `packages/app-lib`): the Modrinth App with Amberite
  added on top. Keep upstream behavior and UI. When something must differ, change the API behind
  the UI, not the UI.
- **Backend** (`apps/backend`, Cloudflare Workers + D1): a runnable clone of Modrinth's labrinth
  minus content management. It owns accounts, sessions, friends, presence, and sharing metadata:
  members, versions, where files are stored, and who may download them. It is a coordinator. It
  does not hold shared files. It runs locally with no Cloudflare account or billing.
- **Core** (`apps/core`, also called Copal): the self-hosted server manager. It is a product, not
  the general backend.
- **Modrinth** serves all public content. Shared versions reference Modrinth files by id.

Who talks to whom:

- App to backend and App to Core hosting: through `packages/api-client`, with Modrinth-shaped
  endpoints wherever that makes sense.
- App to Core for hosting screens: Core's hosting API at `/hosting`, shaped like Modrinth's Archon.
  This is the only API Core exposes to the App.
- Sharing uploads and downloads: the existing native path in app-lib. app-lib may be edited for
  this. Keep our additions in separate modules with small hooks into upstream code.

## Sharing and storage

There are no friend groups. You share an instance with friends, and sharing is not tied to a Core.
Someone an instance is shared with does not control it; for now they can only propose changes.

Goal: share an instance with friends, push updates, and have them install even while the owner's
App is closed, without Amberite paying to host files.

- Files that are not on Modrinth (configs, custom mods, datapacks) are stored on **Cores**. The
  storage module runs inside Core. The standalone `sharing-storage` program in app-lib is the
  starting code and a local test tool, not the destination.
- Which Cores: other people's Cores, not only the sharer's own. If the sharer has a Core, prefer
  it. Only always-on Cores count; a Core running from the desktop App is not used. Sharing does
  not require owning a Core.
- Contribution is on by default. Setup does not ask. Limits and opt out live in settings. Opting
  out stops community storage on that Core and nothing else.
- No temporary hosted copy. The owner's App already has the files. It saves metadata first, shows
  "waiting for online Core" until a Core is available, uploads directly, and shows "pushed and
  updated" only after the upload is verified. Both ends verify every transfer.
- One verified copy is enough for friends to install. A second copy is made on another Core when
  one is available.
- The owner's App keeps its uploaded versions so lost files restore automatically when it is
  online again.
- Hosted online: the five latest versions plus versions the owner pins.
- The NAS is separate storage-only overflow. It is not a Core and not a backup archive.
- Moving files between Cores happens on events such as a copy disappearing. Healthy files do not
  get reshuffled.

Not built: storage inside Core, uploading to real Cores, choosing Cores, deleting pruned files,
port forwarding or tunnels, encryption. What works today is the backend plus two local storage
processes: upload, second copy, download with the owner offline, hash checks, retry, restore.

## Privacy

A Core owner must not be able to read other people's files stored on their Core. Encryption is
required eventually and deferred for now, so today's storage is readable by whoever runs it. Do
not describe it as private. Analytics can be opted out of. Private Cores and hiding contributor
addresses are later.

## Linked servers

A server is a hosted world of an instance. One instance can have many. A linked server takes its
loader, game version, and content from the instance and keeps its own world, files, backups, and
server-only additions (mods and datapacks).

- Create a server from Instance > Worlds, which skips to world settings, or pick "From instance"
  in server setup. Anyone who has the instance installed can create one on a Core they control.
  Creating one on someone else's Core is later.
- Pushing an instance update delivers it to every linked server. Core downloads and verifies at
  once and applies only while the server is stopped. Only the latest version is applied.
- Push buttons: "Push and restart" is the primary action. If players are online it asks: restart
  now, apply on next stop, or cancel. "Push update" is secondary and never restarts.
- Client-only content is not installed on the server but is still listed. Unknown custom mods are
  installed. Inherited content cannot be disabled yet.
- Server-only changes are stored on that server's own Core and do not prompt players to update.
- Having control of a server does not give control of the instance's content.
- Update progress and failures show in the App's Tasks panel and notifications.
- Worlds lists linked servers with their state, a Linked tag, and Manage. The server Content page
  shows the instance's content read-only, named as coming from the instance, with the server's
  additions in a separate list. The exact layout is not final.
- Keep the Access page. History views, overrides, branches, and forced resource packs are later.

## Accounts, Core auth, pairing

- Accounts work like Modrinth's, served by the backend.
- Core gets basic authentication against that same account system now. The hosting API must work
  with authentication on; today it only mounts in dev with authentication off.
- Pairing is later. The intent: a Core is linked to an account directly, and several Cores under
  one account are managed as one.

## Current state, 2026-10-10

- Accounts, friends, presence, and sharing between two Apps work locally.
- The hosting screens work against a local Core with authentication off.
- Branch `core-apply`, not merged: the Core engine that installs and updates a linked server from
  a pack or an instance version, with unit tests. Its endpoints sit on Core's second API and need
  to move under hosting. It has not run against real downloads.
- Branches `server-delivery` and `server-ui` hold a migration and a mock data file. They will not
  be continued.

## Cleanup

- Remove Convex: `convex/`, the Convex client inside `packages/api-client`, its tooling, and
  Core's dependency on it. `packages/api-client` itself stays. The last commit where everything
  worked on Convex is `762843d41`.
- Remove Core's second API (`/instances`, `/sync`, `/core`, and the rest) and `CoreApiClient`.
- Remove Core's legacy sync profiles.
- Remove `apps/realtime`. Presence is in the backend.
- Ignore the website (`apps/frontend`). It will be reverted to upstream. The local account login
  page it currently provides needs a replacement before that.

## Open questions

- R2: start with R2 for shared files and move to Cores if cost appears, or go straight to Cores.
- Whether local owner history can be pruned by a setting, and its default.
- Storage budgets per Core, and how Cores are picked.
- How Core proves itself to the backend before pairing exists.
- Tunnels, port forwarding, and how the NAS is packaged once storage lives in Core.
- Enforcing who can join a server, beyond hiding its address.
