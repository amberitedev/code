# Networking & Remote Access — Design Notes (future work)

Captured design ideas for exposing self-hosted Amberite Core instances to the
internet. None of this is implemented yet; it records intent and trade-offs so
we don't lose the reasoning. See the TODOs in
`src/application/instance_status_service.rs` (`start_instance`).

## Problem

Core runs on the user's own machine (single tenant, they own the box). Unlike a
commercial host, Core does **not** control a shared firewall or a pool of node
ports. A Minecraft server needs its game port reachable from the internet, and
many features need extra ports (voice chat UDP 24454, GeyserMC UDP 19132, web
maps 8123, RCON 25575, query). On a home connection these are blocked by the
router/NAT by default, and Core cannot open them on its own.

Because of this, a manual "port allocations" table (like Pyro/Pterodactyl) adds
no value on Core by itself — recording a port does nothing without something
that actually opens it. So allocations are deferred until one of the access
mechanisms below exists.

## Access mechanisms (ranked by effort/value)

### 1. UPnP automatic port mapping
- Ask the router to open/forward the server port automatically at instance start.
- Crate: `igd2`. Best-effort; many routers disable UPnP.
- Lowest friction when it works; silently no-ops when the router refuses.

### 2. playit.gg tunnel (fallback when no inbound connectivity)
- Used **only when a direct/UPnP connection can't be established**. Runs in the
  background and sets up a tunnel for the user automatically (user is aware but
  it's largely hands-off). See https://playit.gg/api-docs
- Good universal fallback behind strict NAT/CGNAT.

### 3. Cloudflare Tunnel (preferred long-term, free)
- `cloudflared` tunnel: no router login required, no inbound port forwarding,
  generous free tier with effectively no limits for this use.
- Especially valuable when the user is locked out of their hosting account /
  router admin and cannot port-forward manually.
- Could back per-server public addressing without touching the firewall at all.

### 4. Optional firewall control
- Core could manage the OS firewall directly. On Windows this is comparatively
  safe (you can't permanently lock yourself out). On managed/remote hosts it's
  risky (lockout). Would also require auto-assigning the server port at creation
  time so users don't have to think about it. Only worth it if it materially
  simplifies the multi-port problem; otherwise tunneling supersedes it.

## Custom subdomains (open question)

The operator owns a domain (e.g. `amberite.dev`) and could hand every server its
own subdomain — `michaelminecraftserver.amberite.dev` — uniqueness enforced
(no two servers share a subdomain). Costs the operator nothing beyond
maintaining the domain. This pairs naturally with Cloudflare Tunnel (each tunnel
maps to a hostname). Pending a product decision on whether to offer it and how
to provision/validate names. Tracked as an end-of-task query.

## Port allocations — revisit later

Once any of the above lands (especially Cloudflare Tunnel + subdomains), the
multi-port "allocations" concept becomes meaningful again (voice/Geyser/map
ports per server). Until then, Core exposes a single primary game port only.
