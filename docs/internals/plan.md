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

**The shared file pool.** The App keeps Modrinth's own sharing code and UI. Our backend answers
the same API with our accounts, and the files live in a pool of always-on Cores. The design and
the exact protocol are in `pool.md`. Storage was decided on 2026-10-28.

- Shared exactly as Modrinth does: links for anything on Modrinth; real files for content that is
  not (mod jars, resource packs, shaders, datapacks); and the config files the owner picks when
  pushing, each under its path in the instance.
- The owner's App encrypts every file and uploads it straight to the share's Core; members
  download from it. The backend keeps the manifest and signs short-lived tickets.
- Every share is stored twice, a home and a replica. An instance with a server has its home on the
  owner's own Core and its replica on a Core friends can reach. Without a server, or without a Core
  of your own, both copies are on other Cores, or on your own Core while no one else's can take
  them. A share with no members (a server-only instance)
  has no replica. The owner's App is the last backup: the next push restores a share that lost its
  Cores.
- Sharing works for everyone. Everyone gets an allowance on other people's storage, 3 GB for now.
  Turning storage off on your Core means you run privately: your shares live only on your own Core
  and you use no one else's, and other people's shares move off it. A Core that friends cannot
  reach is not much use anyway, since they cannot join its servers either.
- Usage is the size of the files you uploaded across the versions kept, each file counted once
  however many copies exist. Only shares on someone else's Core count, so Modrinth files,
  manifests, private shares, and shares with no members on your own Core do not. The limit numbers
  are tuned over time. Also 1 GB per version of a share and 4,096 files per version.
- How many versions are kept, files included, is the owner's auto-cleanup setting: keep the last
  five by default, any other number, or all of them. Older versions drop on their own. A push over
  the limit is refused with "delete old versions to free space". Usage, the setting, and the kept
  versions, with a way to delete them, are in Settings, Cores, beside each Core's storage.
- A push with no Core offering storage fails with a clear message. Storage is on by default on a
  Core and is turned off, or its caps changed, in Settings, Cores.
- Two packages: `sharing-client`, the share format and encryption the App and Core both use; and
  `sharing-storage`, the storage process. It stays a child process of `copal` on the Core's address
  and port, reports through the Core, and talks to it over a small local API. It has no identity
  of its own: the backend knows one Core. Its own OS account comes with the installer.

Later, in rough order of intent:

- Show in the push UI which files are over a limit, before pushing.
- Pins, once a version history view exists. KubeJS scripts in the bundle.
- Stop uploading files at all where possible: reference where a file already lives and make sure
  it stays there, so a shared version is about half a megabyte.
- Re-homing when a Core holding a share goes away, as `pool.md` describes.
- Limits tied to what you give: offer storage to others (for example 10 GB) and your own
  allowance follows from it.

## Privacy

Shared files are encrypted before they leave the owner's App, so a Core owner cannot read what
their Core stores. The backend holds the keys, as it already holds accounts. The one exception is
a Core applying a version to a server: its owner has access to that share, so the Core is given
the manifest and decrypts that version like a member's App. Analytics can be
opted out of.

## Linked servers

A server is a hosted world of an instance. One instance can have many. A linked server takes its
loader, game version, and content from the instance and keeps its own world, files, backups, and
its own additions (mods and datapacks added on the server itself).

How it works: a server is one more consumer of the instance's sharing, like a member. It reads the
same versions members read, from the same place. Nothing is ever uploaded to a server directly.

- Every instance with a linked server publishes versions through sharing, even when it is shared
  with nobody. A push sends the manifest to the backend and only the new files to the store, as
  any share push does.
- Push and restart hands each server's Core the share's latest version as the backend gave it to
  the App: a Modrinth pack index, the pool entries of the other files, and a fetch ticket. The
  Core downloads Modrinth files from Modrinth and the rest through the store API, checks and
  decrypts them as a member's App does, and applies only what changed. A running server takes it
  when it stops, or is stopped, updated, and started again.
- The Core reaches files only through the store API. How and where the store keeps files does
  not matter to it, so the store can become its own package without changing this.
- Each server records the version it applied. The owner sees when a server is behind the latest
  push, and server cards follow the applied version.
- A push that only changes what servers use (server-only mods, server config) is still a normal
  version, but members are not prompted. A member's App compares its content with the latest
  version, ignoring server-only mods, and offers no update when nothing it needs changed; it
  takes that version with the next update that matters to it.
- Anyone the instance is shared with can host from it, because a server only needs read access to
  the share's versions.
- A server gets the configs a version carries, nothing more. Any other config it generates itself.
- An instance with a linked server is stored on its owner's own Core, even when that Core is not
  eligible to store for others, and storage there does not count toward the sharing limits. This
  is storage work and ships on its own; servers work with any placement, because the Core fetches
  from wherever the backend points it.

- Create a server from Instance > Worlds, which opens server setup at its last step (world
  settings), or pick "From instance" in server setup. Only on a Core you own.
- Push: an instance with a linked server has one push button, "Push and restart". If players are
  online it asks: restart now, apply on next stop, or cancel. It publishes one version, which
  friends and servers read. An instance with no friends and no server has no push button.
- Core applies an update only while the server is stopped. A push to a running server waits and
  applies when it stops. Only the latest pushed state is applied.
- If the Core is unreachable when pushing, the server is not updated; the App says so, and the
  server's Content page has "Update from instance" to do it later.
- Client-only content is not installed on the server but is still listed. Unknown custom mods are
  installed. Inherited content cannot be disabled yet.
- An update removes only files the instance provided before. Additions made on the server are
  never touched.
- Core takes and reports the instance through the hosting API's existing content and modpack
  routes (install content with `content_variant: 'instance'`, unlink, update, the content list
  with its linked pack and from-pack flags), so an instance is one more kind of pack source. There
  are no separate linked-server endpoints.
- Worlds lists linked servers with a Linked tag, their state (running, offline, updating), and
  Manage. The server Content page shows the instance's content read-only in Modrinth's modpack
  card, titled with the instance's name, with the server's additions in a separate list. Unlink
  is in the card's menu. A server can be deleted.

Later:

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
  "Open Amberite, go to Settings > Cores > Add Core, and enter ABCD-EFGH", and waits on a socket
  that the backend wakes on approval, so a Core nobody approves costs almost nothing. You enter
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
  address when it listens on every interface. Nobody types an address. It must be HTTPS unless it
  is loopback or a private network address. Once the Core has an address on the internet it reports
  that too (see "Reaching a Core from outside").
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
- Outside test: a granted mapping is not proof. The backend no longer tests ports.
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
  delete it. Servers follow the instance's share, as "Linked servers" describes.
- Known gaps: an over-limit push shows a generic error; the App's network allowlist is opened to
  Core addresses only in the dev runner, not in a packaged build (a packaged build does allow the
  local port the App opens for a Core that serves HTTPS).

## Open questions
