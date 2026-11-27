# Amberite Connect: reach any App or Core with no port forwarded

Status: the foundation is built (the `connect` crate, and the App managing its Core through it).
Friends joining, stores, world hand-off and LAN worlds are not. Lodestone Connect's design,
written by us: the backend introduces two programs, they find a path through their routers, and
every byte after that goes directly between them. Nothing is ever relayed. Earlier drafts used
iroh and a relay server; both are dropped (see "Why not iroh").

## What needs a connection

| Need                          | From                      | To          | Traffic                             |
| ----------------------------- | ------------------------- | ----------- | ----------------------------------- |
| Manage your Core              | App                       | own Core    | small HTTP, console socket, uploads |
| Join a friend's server        | friend's App              | Core        | Minecraft TCP; voice mods add UDP   |
| Download a shared pack        | friend's App, other Cores | a store     | bulk                                |
| World hand-off (host rotates) | App or Core               | App or Core | bulk, 100 MB to a few GB            |
| Open-to-LAN singleplayer      | friend's App              | host's App  | Minecraft TCP                       |

The foundation is: **ask the backend to introduce you to a program by its id, get a direct
encrypted connection, and know who is on the other end.** Each feature is a protocol on top.

## How a connection is made

1. **Identity.** Every Core and every App has a key and a self-signed certificate. Its
   fingerprint is its id. Core already has one and the App already pins it; an App makes one per
   run (mutual TLS inside QUIC). A feature that needs a device to keep its id, such as a join
   ticket bound to it, has to store the App's.
2. **Waiting to be reached.** A Core keeps one WebSocket open to its own Durable Object on the
   backend. It hibernates: Cloudflare answers its pings without waking the object, so an idle
   Core costs nothing. This also gives the backend a way to tell a Core something, which it lacks
   today.
3. **Candidates.** Each side lists where it might be reached on its one UDP port: its LAN
   addresses, IPv6, its public address as STUN sees it (`network/public_ip.rs` already asks
   Cloudflare's and Google's STUN), and a port the router mapped over PCP/NAT-PMP/UPnP
   (`network/router.rs`, which gains UDP mappings).
4. **Introduction.** The dialer asks the backend to connect to a Core for a purpose (manage, join
   server S, fetch from a store). The backend checks the dialer may, then sends both sides each
   other's candidates and ids through the Durable Object.
5. **Punching.** The listener sends small UDP packets at the dialer's candidates while the
   dialer starts a QUIC handshake at each of the listener's. Each router takes the other side's
   packets for replies and lets them in. The first handshake to finish wins.
6. **The connection.** QUIC (quinn, already in our lockfile) on that path, TLS pinned to both ids.
   Streams inside it carry the protocols below.

**Invisible to everyone else.** A Core's UDP port drops every packet from an IP address it was
not introduced to, before QUIC sees it. It goes by IP, not port, because a strict router gives
the dialer another port than the one it offered. A scanner finds nothing. A server shared this way is
invite-only at the network level, which a forwarded Minecraft port can never be: bots find those
within minutes. Friends do learn the host's IP, as with any direct connection.

**Same network, internet down.** The App tries the Core's LAN address first, with no introduction.
The gate lets private addresses through, and the API still requires a session.

**When it fails.** Two strict routers, and no mapped port on the Core's side, have no path. There
is no fallback. Today the App shows the Core as unreachable; saying why and giving the fix (turn
on UPnP, or forward one port) is still to build. Because the
Core maps its port automatically first, a Core with a working router is reachable from any friend,
whatever the friend's network. Hole-punch success between two unmapped home routers is commonly
measured at 70-90%.

Vanilla friends without Amberite keep using a forwarded game port, as today.

### Why not iroh

iroh solves this plus relaying, and in iroh the relay server is also what sets up the hole punch.
With no relaying, we'd still have to host or rent that server. Our version needs only the
backend we already run, and the parts iroh would save us (switching between relayed and direct
paths) are the parts we don't want. What we write: candidate gathering (most exists), the
introduction, the probe loop, and the UDP gate. Lodestone is AGPL, so we build from the design and
copy no code.

## Protocols on top

QUIC agrees on one ALPN per connection, so each protocol gets its own connection, with a stream
per use:

| ALPN               | Carries                                                                                                                                     | When  |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- | ----- |
| `amberite/api/1`   | the Core's HTTP API, served directly (`api::serve_connection` takes any stream)                                                             | built |
| `amberite/game/1`  | a length-prefixed header naming a server (never a port), then raw Minecraft TCP; versioned so voice mods' UDP can ride QUIC datagrams later | next  |
| `amberite/store/1` | the store's HTTP, on the store's own endpoint                                                                                               | later |
| `amberite/world/1` | world hand-off                                                                                                                              | later |
| `amberite/lan/1`   | open-to-LAN worlds, App to App                                                                                                              | later |

Nothing in the connection layer assumes one side is a Core: world hand-off and LAN sharing dial
Apps.

## The three uses ilai named

### 1. Managing your Core over Connect

Built. The App reaches its Core only through Connect. `address` is the LAN hint, and the App's
TLS-over-TCP path is gone. The Core serves its API straight on the stream, so it sees each
caller's real address and its sign-in rate limit keeps working (through a loopback pipe every
caller would be `127.0.0.1`, and one friend could lock the owner out). Auth stays the sessions the
Core has today, so no backend call per connection. The account session never leaves the WebView,
so the WebView carries the introduction between the App's Rust side and the backend, and waits
longer each time before asking for a Core that stays off.

Automatic port forwarding comes first, as ilai wants: the Core maps its UDP port the way it maps
game ports. Hole punching covers routers that refuse.

### 2. World hand-off for people without a Core

A friend group shares a world. Whoever hosts tonight takes it; the world moves to whoever hosts
next. Nobody needs an always-on machine: hosting runs on the Core the App sets up on that PC ("Set
up a Core on this PC"), so the App never needs its own server code.

- **Hosting lock.** A row on the backend: who hosts, since when. Taking it is a compare-and-set, so
  two friends can never fork the world. Two writes per session. A host that disappears loses it
  after a timeout, and the host fences itself by its own clock: it renews its lease while it runs
  the world, and stops the world once its last confirmed renewal is older than the timeout minus a
  margin, so a renewal that hangs or fails cannot leave it running after the backend gives the
  lock away.
- **Versions.** When a session ends, the host snapshots the world as content-addressed files.
  Region files change a few at a time, so most of a hand-off is already on the next host.
- **Moving it.** Over `world/1` from whoever has the newest version and is online.
- **The real limit:** the person with the newest version must be online when the next host starts.
  The fix that costs us nothing is the store pool: the finished version is pushed, encrypted, to a
  store, exactly like a share version, so the next host can pull it while the last one sleeps.
  That ties world hand-off to stores (below), which is why stores matter here.

The foundation needs nothing extra for this: Apps and Cores dial each other symmetrically, and the
backend can introduce any two.

### 3. Stores behind one address: the harsh review

ilai's idea: stores are separate from Cores so their auth never mixes; nobody can target a store
directly; the backend sends you to one with a ticket; everything is "rooted through my domain"; file
bytes never pass through the backend.

**What already exists.** Most of it. The backend already picks the store for a share and signs
tickets for it (`apps/backend/src/sharing/pool.ts`, `cores/tickets.ts`). A store already runs as its
own process, `copal store`, with its own routes and ticket checks. What's missing is reachability:
today only Cores with a forwarded public address may hold other people's shares.

**The part that doesn't work: "rooted through amberite.dev".** A request to a URL on our domain is
answered by whoever the domain points at. For the store's bytes to come from the store, something
on our domain has to carry them. The ways to do that:

- **A Worker proxying to the store.** The bytes pass through the backend, which is the one rule.
  It also can't work: a Worker can't fetch a raw IP, and can't reach a Core behind NAT at all.
- **Cloudflare Tunnel (what T3 Connect does).** Each store runs `cloudflared` and gets a route under
  our domain. It is free and needs no software on the downloader's side. But Cloudflare's terms say
  serving large files through its network needs their paid products, and the account at risk is
  the one the whole backend runs on. That gamble is not worth taking for mod jars and worlds.

So "you can only reach a store through our domain" and "bytes never touch our side" contradict
each other. Pick the second.

**What to keep: "you can't target a store".** It holds in a stronger form without the domain:

- **A store gets its own Connect id, separate from its Core's.** Knowing a store never tells you
  where the Core's API is, and a store connection can never reach the API. That's the auth
  separation, enforced by keys rather than by a rule.
- **A store answers only introduced peers.** The UDP gate drops everything else. The only way to
  reach a store is to ask the backend, which checks the ticket and introduces you for that one
  fetch. You learn its IP for that connection, but you can't come back later without another
  introduction, and nobody can scan for stores.
- **The backend never names stores in anything it returns,** only "connect through me to the store
  for this file".

**How a fetch works.** The App asks the backend for a share version. The backend picks a store that
holds it, signs a fetch ticket, and introduces the App to that store's id. They punch, open
`store/1`, and the App downloads directly with the ticket. Uploads work the same way with a store
ticket.

**Choosing stores that work.** With no relay, a store behind a strict router with no mapped port
would fail for many downloaders. Cores report whether their UDP port is mapped or public, and the
backend prefers those as stores for other people's shares, the way `pool.ts` requires a public
address today, but now automatic instead of a manual port forward.

**Cost.** Per download: one Worker request and a few Durable Object messages for the introduction.
No bytes. The rule that bytes never touch the backend holds, so stores stay cheaper than R2.

**What it gives up.** The downloader learns the store's IP for the length of a download. Hiding it
is only possible by carrying the bytes, which is what we refuse to pay for.

**Verdict.** Good idea once "rooted through my domain" is dropped. It belongs after friends joining, as
`store/1` on the store's own endpoint, and it removes the forwarded-port requirement from stores.

## What comes next

ilai is moving playing without port forwarding into the alpha (on the roadmap's own branch), so
friends joining ships before it.

**Friends join with no port forwarded.** `game/1`, a `127.0.0.1:<port>` per shared server in
the friend's App, the server list entry, `LinkedServer` carrying the Core's id and server id.
Joining is authorized at introduction (only friends the server is shared with). Unsharing reaches
the Core through its Durable Object, which closes that friend's connections; the App closes its
listener. Paper's `connection-throttle` is turned off in `bukkit.yml`, because every friend appears
as `127.0.0.1`. Tests: an unshared friend is not introduced, and is disconnected on unshare. The
Core needs per-peer connection caps then: today only its owner's devices and its own network get
in, so it has one cap for everyone.

Not built in the foundation, and worth doing with it: the App saying why a Core cannot be reached
(two strict routers) with the fix, instead of showing it as offline.

Later, each on its own: stores over Connect, world hand-off, open-to-LAN sharing.

## Cost

Only introductions cost anything. Idle Cores cost nothing (hibernation), and no bytes pass through
us.

Assume 5 connections per user per day, each about 1 Worker request plus 3 Durable Object messages.

| Monthly users | Requests per month | Cost                                |
| ------------- | ------------------ | ----------------------------------- |
| 1k            | ~0.6M              | $0 (free plan: 100k requests a day) |
| 10k           | ~6M                | $5 (paid plan, includes 10M)        |
| 100k          | ~60M               | ~$20                                |
| 1M            | ~600M              | ~$150                               |

It grows linearly with connections made, not with play time or bytes. Reconnects after a
Cloudflare deploy wake every Core's socket at once, which is a burst of requests, not a cost
problem.
