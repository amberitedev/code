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
  members, versions, where files are stored, and who may download them. Shared files live on Cores (see `pool.md`); the backend keeps manifests in R2 and never carries a file byte. It runs locally with no Cloudflare account or billing.
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

**Now: the shared file pool.** The App keeps Modrinth's own sharing code and UI. Our backend answers
the same API with our accounts, and the files live in a pool of always-on Cores. The design and
the exact protocol are in `pool.md`.

- Shared exactly as Modrinth does: links for anything on Modrinth; real files for content that is
  not (mod jars, resource packs, shaders, datapacks); and the config files the owner picks when
  pushing, each under its path in the instance.
- The owner's App encrypts every file and uploads it straight to the share's Core; members
  download from it. The backend keeps the manifest and signs short-lived tickets.
- Limits: 1 GB per shared instance, 3 GB per account across its shares, 4,096 files per version.
- The five latest manifests are kept; Cores keep the files of the latest version only.
- A push with no Core offering storage fails with a clear message. Storage is on by default on a
  Core and is turned off, or its caps changed, in Settings, Cores.

Later, in rough order of intent:

- Show in the push UI which files are over a limit, before pushing.
- Pins, once a version history view exists. KubeJS scripts in the bundle.
- Stop uploading files at all where possible: reference where a file already lives and make sure
  it stays there, so a shared version is about half a megabyte.
- A second Core in the pool: replicas and re-homing, as `pool.md` describes for phase two.
- The store under its own OS account on an installed Core.

## Privacy

Shared files are encrypted before they leave the owner's App, so a Core owner cannot read what
their Core stores. The backend holds the keys, as it already holds accounts. Analytics can be
opted out of.

## Linked servers

A server is a hosted world of an instance. One instance can have many. A linked server takes its
loader, game version, and content from the instance and keeps its own world, files, backups, and
its own additions (mods and datapacks added on the server itself).

How it works: the App talks to the Core directly, and the backend is never involved. The App
exports the instance as a `.mrpack` and uploads it to the Core through the hosting API. Core's
engine installs it, and on a later upload applies only what changed. This works for instances
that are not shared with anyone and uses none of our storage.

- Create a server from Instance > Worlds, which opens server setup at its last step (world
  settings), or pick "From instance" in server setup. Only on a Core you own.
- Push: an instance with a linked server has the push buttons. "Push and restart" is the primary
  action. If players are online it asks: restart now, apply on next stop, or cancel. "Push update"
  is secondary and never restarts. If the instance is also shared, the same push also publishes to
  friends. An instance with no friends and no server has no push button.
- Core applies an update only while the server is stopped. A push to a running server waits and
  applies when it stops. Only the latest pushed state is applied.
- If the Core is unreachable when pushing, the server is not updated; the App says so, and the
  server's Content page has "Update from instance" to do it later.
- Client-only content is not installed on the server but is still listed. Unknown custom mods are
  installed. Inherited content cannot be disabled yet.
- An update removes only files the instance provided before. Additions made on the server are
  never touched.
- Core reports the instance through the hosting API's existing content and modpack routes
  (install content, unlink, update, the content list with its linked pack and from-pack flags), so
  an instance is one more kind of pack source. There are no separate linked-server endpoints.
- Worlds lists linked servers with a Linked tag, their state (running, offline, updating), and
  Manage. The server Content page shows the instance's content read-only in Modrinth's modpack
  card, titled with the instance's name, with the server's additions in a separate list. Unlink
  is in the card's menu. A server can be deleted.

Later:

- Skip publishing to friends when a change only matters to the server (only server-side mods
  changed), so server-only changes use no backend storage and prompt nobody.
- Friends see an instance's servers in Worlds, from a small list of name and address kept with
  the shared instance.
- Update progress in the Tasks panel. History views, overrides, branches, forced resource packs,
  and creating a server on someone else's Core.

## Accounts and Core sign-in

- Accounts work like Modrinth's, served by the backend.
- Core checks account tokens against that same account system. The hosting API is always on and
  requires login.
- A Core belongs to one account, its owner. Owner only: no members, roles, or invites.
- A Core signs in like a device: it asks the backend for a code and the key it will use, prints
  "Open Amberite, go to Settings > Cores > Add Core, and enter ABCD-EFGH", and polls. You enter
  the code in the App and approve it; the App shows the Core's machine name first so you can tell
  it is yours. From then on the key works, the Core is listed under your account, and the Core
  prints "Signed in as <you>". The App never talks to the Core during sign-in, so
  approving sends your account session nowhere; once the Core is listed, the App sends it your
  session like any other Core on your list. A code works for 15 minutes; a Core has one code at a
  time.
- The Core does this on its own on first start, and `copal auth` does the same from the command
  line, for a Core installed as a service or reached over SSH. Both show one code: the code is
  kept in the Core's database so a running Core and `copal auth` do not replace each other's. From
  a terminal on a computer with the App, the Core also opens `amberite://approve/<code>`, which
  opens Add Core with the code filled in.
- The Core's key is a Core key, not a session. With it the Core reports its address, certificate,
  and version, lists the servers it shows to friends, invites and removes people on its servers,
  and asks for reachability tests. It cannot touch friends, sharing, notifications, or the
  account; those routes take account sessions only.
- The backend keeps each account's list of Cores and where they are, so the App finds them on any
  computer you sign in on. After sign-in the App talks to the Core directly.
- A Core reports its own address, on every start, every five minutes after that, and whenever it
  changes. By default that is the address and port it listens on, or the machine's local network
  address when it listens on every interface. `AMBERITE_PUBLIC_URL` overrides it. Nobody types an
  address. It must be HTTPS unless it is loopback or a private network address. Once the Core has
  an address on the internet it reports that too (see "Reaching a Core from outside").
- A Core makes its own certificate on first start and serves its API over HTTPS with it. No domain
  and no certificate authority. The Core sends the certificate's SHA-256 fingerprint when it asks
  for a code, the backend stores it with the Core, and the App trusts only that certificate for
  that Core. The WebView cannot check a certificate like that, so the App's Rust side opens a port
  on the App's own computer for each Core and carries every request, upload, and WebSocket to the
  Core over TLS, checking the fingerprint.
- The same port still answers plain HTTP, but only to callers on the Core's own computer or local
  network, so local tools keep working. A Core signed in before this reports its fingerprint with
  its key the next time it starts; until then the App reaches it the old way.
- An account can have many Cores, treated as one: their servers form one list, and creating a
  server asks which Core only when there is more than one.
- Remove is in the App under Settings, Cores. It is one call to the backend: the key stops working
  at once. The Core finds out on its next call, within five minutes, forgets its owner and key,
  refuses hosting calls, and shows a new code. Its servers and files stay. `copal logout` does the
  same from the Core's side.
- Limits, all on the backend: 5 wrong codes per account in 10 minutes; code requests are limited
  per Core and per caller address.
- A Core that was owned before Cores had keys counts as signed out and signs in once.

## Reaching a Core from outside

A Core at home must be reachable for two things: friends joining the Minecraft server, and the
App managing the Core from anywhere.

- Core forwards its ports and opens the local firewall automatically.
- When that fails, Core's command line explains in detail where the problem is and how to fix it.
- Game traffic never goes through us. No relay and no tunnel for now. The answer for bad routers
  is a robust forwarding system and a clear diagnosis; tunnel support may be added later for
  networks where forwarding is impossible.

How it works (`apps/core/src/network`):

- Forwarding: Core asks the router over PCP, NAT-PMP, then UPnP (`crab_nat` and `igd-next`). One
  TCP port per running server and one for Core's API; none for the API when Core only listens on
  its own computer. A server keeps its port inside the network. Outside, Core asks for the same
  number first and takes whatever the router grants; if a port is refused, or granted but fails
  the outside test, it tries up to two other outside ports.
- Mappings have a one hour lease and are renewed every five minutes, which also restores them
  after a router restart. They are removed when a server stops, never when Core exits, because
  servers outlive Core. Every few minutes Core checks for a changed home address or a different
  network and starts over when it sees one.
- Firewall: on Windows one inbound rule, "Amberite Core", covers Core's port and server ports
  25565 to 25664 on private networks only, so public Wi-Fi stays closed. Core asks for administrator rights once with the normal Windows prompt and never
  again, whatever the answer; after a refusal it prints the command to run. On Linux Core handles
  ufw and firewalld: it opens ports itself when it runs as root and prints the command otherwise.
  On macOS it only reports that the firewall is on. Hand-written nftables or iptables rules are
  not read.
- Outside test: a granted mapping is not proof. The backend no longer tests ports; see the Network
  section of `core-plan.md`.
- Diagnosis: whenever the answer for a port changes, Core prints what it tested, what it found,
  and the steps to fix it. It tells these apart: the server is not listening; the local firewall
  blocks it; the router answers no forwarding protocol; the router refused; two routers in a row;
  the provider shares one address between customers; the router opened the port but nothing
  arrives (the provider blocks it); a public address with a firewall in front. It says so when
  nothing on the owner's side can fix it.
- Addresses: once the API passes the outside test, Core reports `https://<address>:<port>` to the
  backend as its `public_address`. The App uses a Core's local address when it answers and the
  public one otherwise. Each server's outside address and its reachability come back from the
  hosting API with the server (`net.outside`, `reachability`).
- The App: "Copy address" on a linked server copies the outside address when there is one. The
  signal icon on a linked server's row in Worlds turns orange when the server runs but cannot be
  reached from outside, and hovering it shows the cause.
- IPv6 is not handled yet.
- After this exists: friends see an instance's servers, and the owner can invite friends to
  control a server.
- Who can join a server is the owner's choice: a whitelist or none.

## Current state, 2026-10-12

- Accounts, friends, presence, and sharing through the pool between two Apps work locally.
- Core has login against our accounts, one API (hosting), and the engine that installs and updates
  a server from a pack. Convex, Core's second API, the legacy sync, and `apps/realtime` are gone.
  The last commit where everything worked on Convex is `762843d41`.
- The website is upstream Modrinth, used only as the local sign-in page.
- Not yet checked on screen by anyone: the merged result of the sharing and Core work together.
- Core sign-in is built: backend list of Cores, sign-in by approved code, the "Add Core" modal,
  `copal auth` and `copal logout`, and Settings, Cores. The dev setting that held the Core address
  is gone.
- Linked servers are built: create a server from an instance, push updates to it, unlink it, and
  delete it.
- Known gaps: an over-limit push shows a generic error; the App's network allowlist is opened to
  Core addresses only in the dev runner, not in a packaged build (a packaged build does allow the
  local port the App opens for a Core that serves HTTPS).

## Open questions
