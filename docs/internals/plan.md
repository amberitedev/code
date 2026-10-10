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
  members, versions, where files are stored, and who may download them. Shared files live in R2 behind it. It runs locally with no Cloudflare account or billing.
- **Core** (`apps/core`, also called Copal): the self-hosted server manager. It is a product, not
  the general backend.
- **Modrinth** serves all public content. Shared versions reference Modrinth files by id.

Who talks to whom:

- App to backend and App to Core hosting: through `packages/api-client`, with Modrinth-shaped
  endpoints wherever that makes sense.
- App to Core is direct. The backend is never in that path. Core has two separate APIs:
  - **Hosting API** at `/hosting`, shaped like Modrinth's Archon. Only the Core's owner can use it.
    Nobody else can access a server for now; sharing a server with friends is a later overhaul.
  - **Sharing API**, later, for when Cores store shared files. It is separate from hosting and has
    its own access: the backend's metadata says who may store or fetch a file.
- Sharing uploads and downloads go through the existing native path in app-lib, kept as close to
  upstream as possible.

## Sharing and storage

There are no friend groups. You share an instance with friends, and sharing is not tied to a Core.
Someone an instance is shared with does not control it; for now they can only propose changes.

**Now: recreate Modrinth's sharing backend on R2.** The App keeps Modrinth's own sharing code and
UI. Our backend answers the same API, with our accounts, and stores uploaded files in R2. Nothing
custom on top.

- Shared exactly as Modrinth does: links for anything on Modrinth; real files for content that is
  not (mod jars, resource packs, shaders, datapacks); and the config bundle the owner picks when
  pushing.
- Limits, much tighter than Modrinth's and easy to change: 25 MB per file, 20 uploaded files per
  version, 5 MB config bundle, 100 MB stored per shared instance. An unchanged file is stored once.
- A file over a limit fails the push the way Modrinth's flow already reports a rejected upload.
- The five latest versions are kept. Files are deleted from R2 when their version is pruned.
- R2 runs locally through wrangler's emulation. No Cloudflare account, card, or hosting yet.
- The Amberite-only upload code is removed: the `sharing-storage` program, the two local storage
  processes, local copies of pushed versions, and resumable uploads.

Later, in rough order of intent:

- Show in the push UI which files are over a limit, before pushing.
- Pins, once a version history view exists. KubeJS scripts in the bundle.
- Stop uploading files at all where possible: reference where a file already lives and make sure
  it stays there, so a shared version is about half a megabyte.
- Large files not held in R2 are listed as needed and pulled from another App that has them.
- If you own a Core, share to it directly so friends fetch from your Core. Further out, files on
  other people's always-on Cores (never desktop Cores), contribution on by default with opt out
  in settings, a NAS as overflow, and encryption so a Core owner cannot read others' files.

## Privacy

Files in R2 are readable by whoever operates the backend. Once files live on other people's Cores,
a Core owner must not be able to read them; that needs encryption, which is deferred. Analytics
can be opted out of.

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
- Linked servers use the hosting API's existing content and modpack routes (install content,
  unlink, update, the content list with its linked pack and from-pack flags). An instance is one
  more kind of pack source. There are no separate linked-server endpoints.
- Update progress and failures show in the App's Tasks panel and notifications.
- Worlds lists linked servers with their state, a Linked tag, and Manage. The server Content page
  shows the instance's content read-only, named as coming from the instance, with the server's
  additions in a separate list. The exact layout is not final.
- Keep the Access page. History views, overrides, branches, and forced resource packs are later.

## Accounts, Core auth, pairing

- Accounts work like Modrinth's, served by the backend.
- Core gets basic authentication against that same account system now. The hosting API must work
  with authentication on; today it only mounts in dev with authentication off.
- For now, the first account to connect to a fresh Core becomes its owner. This is temporary
  until pairing exists.
- Pairing is later. The intent: a Core is linked to an account directly, and several Cores under
  one account are managed as one.

## Current state, 2026-10-10

- Accounts, friends, presence, and sharing between two Apps work locally.
- The hosting screens work against a local Core with authentication off.
- Branch `core-apply`, not merged: the Core engine that installs and updates a linked server from
  a pack or an instance version, with unit tests. It added its own endpoints on Core's second API;
  those go away and the engine is driven by the hosting routes instead. It has not run against real downloads.
- Branches `server-delivery` and `server-ui` hold a migration and a mock data file. They will not
  be continued.

## Cleanup

- Remove Convex: `convex/`, the Convex client inside `packages/api-client`, its tooling, and
  Core's dependency on it. `packages/api-client` itself stays. The last commit where everything
  worked on Convex is `762843d41`.
- Remove Core's second API (`/instances`, `/sync`, `/core`, and the rest) and `CoreApiClient`.
- Remove Core's legacy sync profiles.
- Remove `apps/realtime`. Presence is in the backend.
- Reset the website (`apps/frontend`) to upstream Modrinth. It is only used as the local sign-in
  page, pointed at our backend.

## Open questions

- How Core proves itself to the backend before pairing exists.
- Whether the free Workers plan is enough for R2, to check before hosting.
- Enforcing who can join a server, beyond hiding its address.
