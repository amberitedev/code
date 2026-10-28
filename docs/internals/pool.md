# Shared file pool: design

## Problem

Amberite's sharing copies Modrinth's: a shared instance carries links to Modrinth content, plus real
files for anything not on Modrinth, up to 4,096 files per version. Today those files go to
Cloudflare R2 behind the backend. That cannot be hosted: R2's free allowance is 10 GB, and the upload
route hashes files inside the Worker, which exceeds the free plan's CPU limit on anything larger
than a few megabytes. Paying scales linearly with what people share, forever.

The constraint is zero cost on Cloudflare: the free Workers plan, meaning 100,000 requests and
100,000 database row writes a day, a 500 MB database, 10 ms of CPU per request. A card is on file,
so R2 may hold small things, but no file byte ever touches Cloudflare. Non-negotiables inherited
from the project: the Modrinth App's sharing code and UI stay, sharing works for everyone whether or
not they own a Core, the backend stays the only authority on who may access a share, and files
stored on someone else's machine must be unreadable to that machine's owner.

## Shape in one paragraph

Shared files live in a pool of always-on Cores. The backend keeps the manifests, as it does now,
plus which Cores hold which versions, and signs short-lived tickets. It never carries a file byte.
The owner's App encrypts its files and uploads them straight to one Core; that Core copies them to a
second. A member's App downloads from one of those two. Cores store opaque encrypted objects named
by hash and serve them to anyone presenting a valid ticket; they know no users, shares, or file
names.

## How it works

### Names

Every file has three names, and keeping them apart is the design.

- **P**, the SHA-256 of the plain file. It is what a member checks after decrypting, and for content
  files it is where the key comes from. Only the backend and members ever see it.
- **C**, the SHA-256 of the encrypted file. It is the object's name on disk. Everyone sees it,
  including Cores.
- **R**, the SHA-256 of a version's sorted list of C's. It names the version. Tickets sign it.

### Keys

Files split by the `file_type` Modrinth's App already assigns. Content files, meaning mods,
resource packs, shaders, and datapacks, are encrypted with a key derived from the file itself:
`HKDF-SHA256(P, "amberite-shared-file-v1")`. The same jar encrypts to the same bytes under the same
name whoever uploads it, so the pool stores one copy of a CurseForge mod that a hundred packs
include, with no key exchange and nothing to manage. The known cost of this choice, called
convergent encryption, is that anyone already holding a file can confirm a Core stores it. For
public mod jars that reveals nothing.

Every other file gets a random 32-byte key, stored in its manifest entry next to P. This is not
optional. A config is mostly template, and a Core owner who knows the template can guess the rest
offline against their own disk: a server address in `servers.dat`, a voice chat password, a webhook
token, an RCON password. With a random key there is nothing to guess against. Configs are kilobytes,
so losing their dedupe across people costs nothing.

An unchanged file keeps its key and its C across pushes. The owner's App fetches the previous
version's manifest from the backend, reuses key and C for every file whose P is unchanged, and mints
a key only for new P's. That is what makes version two of a share that changes one config cost one
object, and it works from any of the owner's machines because the backend's copy is the source.

### Primitives

XChaCha20-Poly1305 in 1 MiB chunks, the chunk index as nonce, a last-chunk flag in the associated
data so truncation fails at the cipher and not only at the P check. A member verifies each chunk as
it arrives and a download resumes at the chunk. Modrinth's sha1 stays in the manifest for the
install dedupe that exists today and is never used as a key.

### Push

The owner's App hashes, encrypts, and hashes again, producing P and C per file and R for the
version. It sends the backend only the manifest: names, types, sizes, P, C, keys for non-content
files, R. A manifest is kilobytes to a megabyte. The backend stores it in R2, writes one D1 row with
R, the version's distinct object bytes, home, and replica, picks or reuses the share's home Core,
and returns a store ticket for it.

The App asks the home which of the C's it already has, then streams a tar of the rest with the
version list first. The home verifies each entry's bytes against its name as it arrives, writes it
to a temporary name, and renames it into place, so a crashed upload never reports a half object as
present. It checks that the list hashes to R, keeps the list, and tells the backend what it now
holds. A retry re-asks which objects the home has and sends the rest, so uploads resume per object
on a residential uplink. The version is ready for members when the home has confirmed; the home
copies to the replica in the background with the replica's own store ticket, and that is tracked
separately so a push never waits on a second machine.

A share has at most one unconfirmed version at a time. A second push while one is in flight is
refused with a clear message. An unconfirmed version older than the store ticket's life is
abandoned and the next push replaces it, so a crashed App never locks a share.

Every version is a full manifest: every path the share holds, with its hashes, like a Git tree over
the pool's content-addressed objects. Content files are the instance's content list, which is whole
in every push anyway. Config files are picked per push, so the App carries the latest ready
version's configs forward into the new manifest unless this push sends them again. Their objects
are already on the home, which keeps the latest ready version, so carrying them uploads nothing. Any
version stands alone, and dropping old manifests never loses a file.

### Install

The member's App asks the backend for the version and receives the manifest, the confirmed holders
with their certificate fingerprints, and one fetch ticket for R. It drops every file it already has
by SHA-1, as Modrinth's install does today, then sends a holder the explicit set of C's it wants. The
Core checks each one is in R's list and streams them. The App checks each object twice: bytes
against C, then decrypted bytes against P. Of the config files, an update writes only those that
differ from the version the member last applied, so a member's own edits to configs the owner did
not change are kept, as when upstream published configs one push at a time.

The two failures mean different things. Bytes that do not hash to C are the Core's fault: the App
discards them, tries the replica, and reports the Core. Bytes that hash to C but decrypt to the
wrong P are the manifest's fault: the replica holds identical bytes, so it is not tried. The App
shows that the share's files are broken and the owner must push again, and the backend records the
report for the owner to see. It does not mark the version broken globally, because the report is
unverifiable and a hostile member could otherwise keep flagging a healthy share. If both holders
fail the share is reported unavailable, as today.

### Tickets

The backend already signs Ed25519 tickets that let an App sign in to a Core, and every Core already
verifies them with the backend's published keys. Every ticket carries a `kind`, and a Core rejects a
ticket whose kind does not match the route. The pool adds three kinds:

- **store**: a Core, a version, a size, and the replica with its own ticket. One hour, single-use.
- **fetch**: a Core and a version. Six hours, reusable until expiry.
- **pull**: a Core, a version, and the source Core with a fetch ticket for it. 24 hours, single-use.

None carries a user. Fetch tickets live a few hours, so a removed member keeps access for at most
that long, matching the revocation property R2 gave. Because verification needs only the keys Cores
already hold, the pool keeps serving with the backend down.

### Where a share lives

> Decided 2026-10-28, not built yet: home on the owner's own Core when the instance has a server,
> replicas now rather than in phase two, kept versions (an owner setting, five by default) keep their files on the Cores, usage
> counts uploaded files across kept versions, and a Core with storage off keeps its owner's shares
> only. `plan.md`, Sharing and storage, has the details; this document is updated when it ships.

Every share lives whole on exactly two Cores, home and replica, and all its versions go to the same
pair. An install is served by one Core, never assembled from pieces across many. The two holder
slots exist in the schema from day one.

Placement happens once, when a share is created: the owner's own Core if it is eligible, otherwise
weighted random by free space among eligible Cores with room. Weighted random rather than most free
so a new large disk does not absorb every share until it is the fullest.

Replica and re-homing are phase two, built when a second Core joins the pool. The rules when they
arrive: a home whose stored bytes fail the C check for three current members of one share in a day
swaps roles with its replica for that share, at most once per share per day. Both holders have the
same bytes, so the swap costs nothing and a few hostile accounts in one share can only shuffle their
own share. A Core with C failures from three distinct shares in a day takes no new placements for a
day. A Core silent for a week is treated as gone, its shares get a new Core for the vacated role,
and the survivor is told to copy them over at its next report. A Core whose owner lowers the cap
below what it holds keeps what it has and takes no new shares.

### Kept versions and cleanup

The backend keeps the last five manifests of a share in R2, for history and the diff view the UI
expects. Each is a full manifest, so nothing depends on the ones dropped. Stores keep objects for the latest confirmed version only; disk is the real resource.

Each Core reports once a day what it holds and how much room it has, and after every stored
upload. The backend answers with versions to drop and versions to pull. A holder's drop list is
computed against the latest version that holder has confirmed, so a replica never drops the
previous version's objects before it holds the new one. Objects are files under a two-level hash
prefix. A version's list file goes with its version, and an object is deleted when no live list
references it, after a grace period equal to the fetch ticket life. There are no packs and no
repacking.

### The store

The store is a second process of the `copal` binary. It runs under its own OS account with its own
folder beside the Core's data, unreadable by the Core and by the account that runs Minecraft
servers. It has no database, no sessions, and no secrets: not the Core's key, not the TLS key. It
holds the backend's public keys, encrypted objects, and a list of C's and sizes per version, which
is the only share-shaped thing it ever sees.

The Core keeps its one port and terminates TLS as it does now. Requests under `/store/` are
forwarded to the store on localhost as opaque streams; the Core never parses them. The store
verifies tickets itself, so it stays the authority on what it serves. It has three routes: store a
version, say which objects you have, fetch objects from a version. It never unpacks or inspects
what it stores. It enforces its own disk cap, a daily egress cap, a concurrent fetch limit, a
per-address limit on refused requests (a fetch with a valid ticket never counts, so a large install
is not slowed), and the size limit from the ticket on each upload, and it answers over cap
with one of two errors, disk full or egress spent, so the App can show the right thing.

The store writes a status file the Core reads, and the Core includes storage in its daily report
with the Core key it already has. A store only ever fetches from another Core that a backend-signed
ticket names, pinned to that Core's certificate, so it cannot be turned into a general downloader.
A compromise of the store process yields no key and no identity.

### Who contributes

A Core is eligible when it is installed as a service, has storage on, has a public address that is
not private, loopback, or link-local, has room under its cap, and has been reporting. Reachability
is confirmed by the first successful fetch from any App. Any operating system; a Windows VPS is as
always-on as a Linux one. A store-only node is a Core with storage on and no servers.

Storage for others is on by default on eligible Cores, offering 20 GB or a tenth of free space,
whichever is smaller, with a 10 GB floor. The owner sees usage, the disk cap, and the egress cap in
Settings, Cores and can change or turn storage off. Turning it off has no consequence for sharing.
People who own no Core are never affected.

### Limits

- 1 GB per share at launch. Modrinth allows 2 GB; the pool starts lower to protect the first disk
  and grows with usage.
- 3 GB per user across all their shares, the sum of each share's latest-version distinct bytes read
  from D1. Checked when a store ticket is issued, as the total minus the share's current version
  plus the new one.
- 50 GB of egress per Core per day and a concurrent fetch limit, so a leaked ticket or a scripted
  install cannot saturate a volunteer's uplink. Over the cap the App tries the replica.

### Launch

The pool is one Core: Ilai's Oracle Cloud always-free VM, with storage on and no servers. Its disk
is about 45 GB with about 24 GB free, so the launch cap is 10 GB. At 1 GB per share worst case that
is ten full shares; realistic shares carry only their non-Modrinth files and are a few hundred
megabytes, so the real number is a few dozen. When it fills, Oracle allows up to 200 GB of free
block storage to be attached. There is no replica and nothing to re-home. A push with no eligible
Core fails with a clear message and never falls back to Cloudflare.

### What the backend costs

Per push: one R2 write, one D1 row, one holdings row per confirming Core. Per install: D1 reads and
one R2 read. Per Core per day: one request, one write if anything changed. Per idle Core: nothing.
D1 holds only small rows, so the 500 MB cap is far away. The write ceiling is far above launch
scale. When it is reached, the exits are Workers Paid at $5 a month for five hundred times the
writes, or a small VPS at about the same price.

## What was considered and rejected

- **Land uploads on R2 and hand off to Cores later.** Puts bytes through the Worker and needs R2
  for file content, which the free tier cannot hold.
- **The backend pushes to Cores.** A Worker cannot pin a self-signed certificate, so the backend
  cannot talk to a Core at all. Everything flows App to Core and Core to Core.
- **The owner uploads to every replica.** Multiplies the owner's upload time by the copy count. The
  home copies instead.
- **Bare hashes as access.** A 256-bit name is unguessable, but hashes spread and a removed member
  keeps them. Tickets restore revocation and keep Cores ignorant of users.
- **Random object names.** Would make the same file different in every share, killing dedupe, and
  would let an uploader store wrong bytes under a popular name. Content hashes give both dedupe and
  verification.
- **Convergent keys for every file.** A template plus a 32-bit unknown is minutes of guessing
  offline. Content files keep convergent keys; everything else is random.
- **Manifests in D1.** A manifest is up to a megabyte and the free database is 500 MB. R2 holds
  them as JSON and the Worker streams them without parsing.
- **Packs on disk, as in Git.** A pack can only be deleted when nothing references any object in
  it, so one popular jar pins a pack of garbage forever and the fix is a repacking subsystem. A pack
  is also one upload unit, so a failed push restarts from zero. Objects as files fix all three.
- **Deltas between versions.** Need a party that sees both plain files, which only the owner's App
  is, and create chains that complicate serving, deletion, and replication.
- **Chunking from day one.** Right idea, deferred: it changes nothing on the Core or in tickets, so
  it can be added later.
- **Diffs by "I hold version N-3".** Needs old version lists kept forever. The member sends the set
  of objects it wants instead.
- **Keeping five versions on stores.** Disk is the real resource. The backend keeps five manifests,
  the stores keep one version.
- **Placement by scoring held objects.** Needs every held version's manifest from every candidate
  inside one 10 ms request. Weighted random by free space costs one query.
- **Per-object holdings on the backend.** One write per object per Core. Holdings are per version.
- **Parity across Cores instead of two copies.** Saves about a quarter of disk, but needs several
  Cores per install and nine reliable Cores before it works at all.
- **A store with its own identity and backend row.** Two things to register, two ports, two
  certificates. The store shares the Core's port and row and holds no secrets.
- **Blocking pushes for Core owners who turn storage off.** Someone with no Core could push, and
  uninstalling the Core would restore the ability. Punishes the friends who were to receive the
  share.
- **A global "broken" state from a P-failure report.** Unverifiable, so a griefing lever against
  owners. The report stays with the reporter and is shown to the owner.

## Threats and answers

| Threat                                          | Answer                                                                                                                           |
| ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| A Core owner reads what they store              | Encrypted with a key Cores never see                                                                                             |
| A Core owner guesses a config from its template | Non-content files use random keys, so there is nothing to guess against                                                          |
| A Core serves wrong bytes                       | Two hash checks in the App; fail over to the replica; C failures feed re-homing                                                  |
| A stranger fetches by guessing                  | Unguessable names, and a fetch needs a signed ticket                                                                             |
| A removed member keeps fetching                 | No new tickets; access ends within hours                                                                                         |
| A stranger fills a Core                         | Storing needs a ticket sized by the backend against the owner's limits; the Core enforces its cap                                |
| A Core's uplink is drained                      | Daily egress cap and concurrent fetch limit per Core                                                                             |
| A Core is used as a downloader                  | It pulls only from a Core named in a backend ticket, pinned to its certificate; private addresses are refused                    |
| An uploader poisons a popular file              | Objects are named by ciphertext hash, so a wrong P breaks only the liar's share                                                  |
| A member griefs an owner with failure reports   | A P-failure report stays with the reporter and is shown to the owner; nothing is marked broken globally                          |
| A few accounts shuffle a Core's shares          | Role swaps are per share and need three current members; Core-wide effects need three distinct shares                            |
| Stored content reaches the host                 | Opaque bytes, never unpacked or executed, in a folder other accounts cannot read                                                 |
| The store process is compromised                | It holds no key and no identity; it cannot impersonate the Core to Apps or the backend                                           |
| The backend is compromised                      | It holds P and keys and can mint tickets, so it can read everything; the same trust it already has for accounts and Core sign-in |
| What a Core learns                              | The address of whoever fetches, never who, and the shape of a version as a list of hashes and sizes                              |

## Open questions

- Fetch tickets are reusable from any address for six hours. The egress cap makes a leaked one
  tolerable; binding to an address would close it at the cost of breaking on mobile and carrier NAT.
- Whether readiness should ever wait for the replica. This design says no.
- The sizes: 1 GB per share, 3 GB per user, 10 GB launch cap, 20 GB default volunteer cap, 50 GB
  daily egress. Set, then revisited from usage.
- The domain.

## Protocol

The exact shapes every side is built against. The backend (`apps/backend/src/sharing`), the
store (`apps/core/src/store`), and the App (`packages/app-lib/src/self_hosted/pool`) share this
text and nothing else.

### Hashes and encryption

- Hashes are SHA-256, written as 64 lowercase hex characters.
- **P** is the hash of the plain file. **C** is the hash of the encrypted object. **R** is the
  hash of the concatenation of the version's distinct C values as raw 32-byte values, sorted
  ascending bytewise (the same order as sorting the hex strings). A version with no files has
  R = SHA-256 of nothing, `e3b0c442…b855`.
- Content files are `file_type` in `mod`, `resourcepack`, `shader`, `datapack`. Their key is
  `HKDF-SHA256(ikm = P as raw bytes, salt = empty, info = "amberite-shared-file-v1")`, 32 bytes.
  Every other `file_type` (`config`, `configs`) gets a random 32-byte key, carried in the manifest
  as `key`, 64 hex characters. A content file carries no `key`.
- Cipher: XChaCha20-Poly1305. The plain file is cut into chunks of 1 MiB (1,048,576 bytes); the
  last chunk is shorter or empty, and an empty file is one empty last chunk. Chunk `i` (from 0)
  uses the 24-byte nonce `i` as a big-endian 64-bit integer followed by 16 zero bytes, and the
  associated data is one byte: `1` for the last chunk, `0` otherwise. The object is the chunks'
  ciphertexts in order, each plaintext length plus 16. So `object_size = size + 16 × max(1,
ceil(size / 1 MiB))`. Encryption is deterministic given the key, so the same plain file and key
  always give the same C, and an upload can be retried by encrypting again.
- A reader verifies each chunk's tag as it arrives, requires every chunk but the last to be
  exactly 1 MiB + 16 bytes, requires the last to carry the flag, and finally checks P.

### Manifest

`external_files[]` on the backend's version routes:

```
{ file_name, file_type, file_size, sha1, sha256, object, object_size, key? }
```

The first four are Modrinth's: `file_size` is the plain byte count and `sha1` is what a member's
install compares to skip a file it already has. `sha256` is P, `object` is C, and `object_size` is
the encrypted byte count. Every version lists all of the share's files (see Push).
`url` is also present for upstream code that expects it and is `pool:<C>`; nothing fetches it.

`POST /v1/instances/:id/versions` (owner) takes the upstream fields plus `external_files` as above
and `root` (R), up to 4,096 files and a 4 MiB body. The backend recomputes R, requires `key`
exactly on non-content files, sums distinct `object_size` into the version's bytes, and checks the
limits. It stores the manifest in R2 as `manifests/<instance>/<version>`, one D1 row, and answers:

```
{ ...manifest, version, ready: false, root, store: { core, ticket, expires } }
```

`core` is `{ id, address, public_address, cert_fingerprint }`. A version with no files is ready
at once and has no `store`. Errors, as `code`: `no_storage` (503, no eligible Core),
`push_in_flight` (409, an unconfirmed version younger than an hour exists), `too_many_files`
(413, over 4,096 files), `share_too_large` (413, over 1 GiB), `storage_quota` (413, over the
owner's 3 GiB across shares). Every 413 names its limit in `description`, which the App shows.

`GET /v1/instances/:id/versions[/:n]` (member) answers the same manifest with `version`, `ready`,
`root`, and:

- `pool: { holders: [core…], ticket }` when ready and the version has files: the confirmed
  holders and a fetch ticket for R, good for six hours.
- `store` again for the owner while the version is unconfirmed, with a fresh store ticket, so a
  crashed push resumes.
- `reports: { content: n }` for the owner: how many members reported bytes that decrypt to the
  wrong P on this version.

`POST /v1/instances/:id/versions/:n/reports` (member) takes `{ core_id, kind }`, `kind` being
`object` (bytes did not hash to C: the Core's fault) or `content` (decrypted to the wrong P: the
manifest's fault). One row per member, version and kind. 204.

### Tickets

All tickets are Ed25519 JWTs (`alg: EdDSA`, `kid` in the header) with `iss: "amberite"`, `iat`,
`exp`, a random `jti`, and a `kind`. A Core rejects a ticket whose `kind` is not the route's.

| kind      | aud                      | claims                                | life                  |
| --------- | ------------------------ | ------------------------------------- | --------------------- |
| `session` | the Core id              | `sub`, `name`, `avatar_url`           | 5 minutes, single use |
| `store`   | the home Core id         | `root`, `bytes` (the version's bytes) | 1 hour                |
| `fetch`   | array of holder Core ids | `root`                                | 6 hours               |
| `pull`    | the replica Core id      | `root`, `source: { core, ticket }`    | 24 hours              |

A store ticket is reusable until it expires, because a resumed upload needs it; it names one
root and a byte cap, so reuse cannot store anything else. The design above says single-use; this
is the decision taken while building.

### Store routes

The store listens on `127.0.0.1` only. The Core forwards every request under `/store/` to it
unchanged, adding `X-Forwarded-For` with the caller's address, and streams the answer back. Every
store route takes `Authorization: Bearer <ticket>`.

- `POST /store/versions/{root}/have`, store ticket. Body `{ objects: [C…] }`. Answers
  `{ missing: [C…], complete }`, `complete` being whether the version's list is already stored.
- `POST /store/versions/{root}`, store ticket. Body: an uncompressed tar stream. The first entry
  is `version.json`, `{ objects: [{ hash, size }…] }` sorted by hash, whose hashes must give
  `root` and whose sizes must sum to at most the ticket's `bytes`. Each later entry is named by
  its C and holds that object; the store hashes it as it arrives, writes it to a temporary name,
  and renames it into place only when the hash matches. Entries the store already holds are
  skipped. When every listed object is present the list is stored and the answer is
  `{ complete: true }`; otherwise `{ complete: false, missing: [C…] }` and the owner sends the
  rest with the same ticket.
- `GET /store/versions/{root}`, fetch ticket. The stored list, `{ objects: [{ hash, size }…] }`.
- `GET /store/versions/{root}/objects/{C}`, fetch ticket. The object's bytes, `Content-Length`
  set, `Range: bytes=<from>-` honoured with 206. C must be in the version's list.
- `POST /store/versions/{root}/pull`, pull ticket. The store fetches the list and the objects it
  lacks from `source` with the fetch ticket inside, pinned to the source's certificate, and
  answers `{ complete }`.

Errors are `{ error, description }` like the Core's: `ticket_invalid`, `ticket_expired`,
`ticket_wrong_core`, `ticket_wrong_kind`, `not_found`, `disk_full` (507), `egress_spent` (429),
`rate_limited` (429), `invalid_request`, `conflict`.

The store also serves admin routes under `/admin/` that the Core never forwards and that take a
token the Core hands the store when it starts it: `GET /admin/status` (`{ cap, used, free,
egress_cap, egress_today, versions: [{ root, bytes }] }`), `POST /admin/drop` (`{ roots }`), and
`POST /admin/config` (`{ cap, egress_cap }`). A dropped list keeps its objects six hours, the
fetch ticket life, before the store deletes those no other list references.

### Core and backend

`POST /core/storage` (Core key) reports storage, after every completed upload and once a day:

```
{ enabled, service, cap, used, free, egress_cap, egress_today, versions: [{ root, bytes }] }
```

`service` is whether this Core is installed as a service. The backend answers
`{ drop: [root…], pull: [{ root, ticket }…] }`. It writes holdings only where they changed, and
confirms every version whose home is this Core and whose root it now holds. A Core is eligible
for new shares when `enabled`, `service`, its public address is not private, `used < cap`, and
it reported within two days. In local development (`LOCAL_DEV`) `service` and the public address
are not required.

The Core's hosting API has `GET /storage` and `PATCH /storage` (owner), `StorageSettings`:
`{ enabled, cap, egress_cap }`, which the App's Settings, Cores page shows and changes.
