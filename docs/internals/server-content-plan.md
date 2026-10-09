# Server content and instance updates

Recorded 2026-10-04 from Ilai's planning conversation. This separates accepted product direction
from engineering recommendations. It does not claim that the features below are implemented.

## Accepted direction

- CPU and RAM percentages use the host's total capacity. Per-server resource limits come later.
- An instance and a modpack both supply a linked server's base installation and content. Reuse
  the same server behavior wherever their source and delivery differences permit it.
- Add "From instance" to server setup alongside the existing modpack, custom, and vanilla paths.
  Creating from Instance > Worlds already selects the instance and skips to world settings.
- One instance can have multiple linked servers. Each server keeps its own world and exceptions.
- Editing the instance prepares changes. Pushing an instance update automatically delivers the
  relevant changes to linked servers; users should not need a second Update action on every server.
- Show delivery and installed-version differences, including pending and failed updates.
- Receive and verify updates immediately, but apply installation/content changes only while the
  Minecraft server is stopped. Enforce this in Core, including start/update races.
- Push UI offers "Push update" and "Push and restart" (see Decided 2026-10-09).
- Normal pushes do not force a running server to restart. The explicit restart choice stops it,
  applies the verified update while stopped, and starts it again.
- The sharing client transfers update bytes directly to a Core/storage service. The sharing
  database records version metadata and file locations.
- For ordinary shared updates, prefer the owner's attached eligible Core. If unavailable, use
  another eligible Core so recipients can still obtain the update. Desktop-integrated Cores are
  excluded from this preference; they do not exist in the current product.
- Server-only updates are stored on the target personal Core only, with no fallback storage on
  unrelated Cores. If it is offline, the author fixes/reconnects the Core and pushes later. Do not
  report a server-only push as delivered before that Core has accepted its bytes.
- Keep direct `.mrpack` upload in Minecraft management distinct from the shared-version path.
  Sharing first retains the authored update locally and transfers it into Core's storage part;
  Minecraft management then consumes the stored version. Do not replace this with direct
  installation as the sharing upload completes. The exact existing local-retention path still
  needs tracing; this is the intended lifecycle.
- Core must support initial installation and updates from the existing shared-version JSON and
  `.mrpack` inputs. Their delivery paths remain separate while their content mechanics are reused.
- Current content work covers mods and datapacks. Server-local additions must not require new
  client content. Forced resource packs are explicitly out of scope.
- Do not add the ability to disable inherited shared content in this section. Client-only
  inherited content is not installed on the server, but stays visible in content API results.
- Filter known client-only content using pack environment and Modrinth metadata. Unknown custom
  mods install by default. Users can disable their local custom additions if those cause issues.
- Physical config-file management and new custom-content support are deferred. Preserve existing
  supported behavior; do not broaden this section to design config conflicts or config overrides.
- Apply only the latest complete pending version, because each version represents the whole setup.
- Integrate update progress, completion and errors into the desktop client's Tasks and Notifications.
  UI variants must be selected before editing real components.
- Worlds lists linked servers alongside the user's existing worlds and added server addresses.
  Linking a server adds its address and name to the instance's server list. Shared recipients
  see the servers they are allowed to access.
- Public means visible and joinable by everyone with access to the shared instance. Private
  means visible only to selected people. Public does not mean internet-wide discovery.
- Keep the Access page. Its controls will be revised later.
- Server control grants no authority to change the parent instance. Instance permissions govern
  changes to inherited content, regardless of the server operator's role.
- History belongs on instances and servers and should show versions and readable change details.
  Reuse the existing update-review presentation, with a larger view to be designed separately.
- Build this in sections, backend first. Design UI alternatives before implementing product UI.

The earlier request to remove Access and the agent's proposal for a manual update action per server
are superseded by these instructions.

## What can be unified

Core should receive a versioned setup with installation choices, inherited files, and environment
selection. Source adapters can supply it from a Modrinth project version, an uploaded pack, or an
instance version. Applying the setup, tracking inherited files, preserving exceptions, reporting
installation status, repair, and unlinking should use common behavior.

Keep source identity distinct. Modrinth projects provide public versions; uploaded packs have no
automatic upstream version source; instances have private access, pushes, and linked recipients.
An instance itself can be based on a Modrinth pack and contain additional content. Its server
must inherit the complete instance version, rather than resolving only that original pack.

## Engineering recommendations, not yet product decisions

- Extend the existing instance version/sharing system to deliver server content. Do not introduce
  another independent instance history or a separate server-sharing product.
- An instance with a linked server needs versions even when it has no invited players. Reuse an
  owner-only instance version record; linking a server must not require inviting people first.
- Record each server's desired version and successfully installed version durably. Notifications
  can prompt a refresh; they must not be the only record that an update is pending.
- Push complete immutable versions. Core downloads and verifies before applying, preserves local
  exceptions, and advances installed version only after the complete update succeeds.
- Keep versions referenced by pending or installed servers retrievable under the existing retention
  policy. Updating a source must not prune the version an offline server still needs.
- Track inherited file ownership and last-applied hashes. Updating configs must distinguish source
  changes from local edits and leave world saves and runtime server files outside the managed setup.
- An offline Core reconciles pending work when it reconnects. A process restart must not lose the
  desired version or an in-progress/failed update.
- Reuse existing authenticated direct-transfer mechanisms and file-location metadata. A Core
  receiving a blob for storage does not by itself authorize applying that blob to a Minecraft
  server; the linked server must accept the source version separately.
- Separate server-only content in the parent instance from an override for one server. Parent
  server-only changes apply to all linked servers; overrides apply only to their target server.
- Server-only payloads remain on the intended personal Core under Ilai's current storage rule.
  Player clients need no download/update prompt when their content is unchanged.
- Instance history shows the source versions. Server history shows which versions it applied,
  application failures, and local overrides. These are distinct views with shared version references.
- A public/private server list is discovery and visibility. Enforcing who can join Minecraft also
  needs a server-side admission mechanism, not merely hiding the address in the launcher.
- Keep dependency compatibility visible when disabling inherited content. File access and console
  control remain powerful: instance ownership can be protected by application APIs, but a host
  controlling the machine can still edit its local files.

## Observed code gaps

- Modpack installation uses `modpack_service` and `infrastructure/minecraft/mrpack.rs`. It installs
  indexed server-compatible files and pack overrides, then stores summary modpack metadata.
- Legacy Core snapshot synchronization uses `social_sync_service` and `sync_apply_service`.
  Its application path plans and updates mods; it does not apply the full installation/config setup.
- A legacy sync profile contains one `core_instance_id`, rather than multiple linked server targets.
- Hosting models label all listed mods `from_modpack: false`. Pack metadata and snapshot records
  are separate, so inherited content does not have reliable shared ownership tracking.
- Core's Hosting content actions support mod addons; other content currently goes through Files.
- The current replacement sharing backend exists in `apps/backend/src/sharing`. It already stores
  numbered versions, readiness, membership, and storage references. The API client exposes these
  through `shared-instances` modules. Older Convex sharing code also remains in the repository.
- Existing push review UI is `SharedInstancePublishModal.vue`; do not assume its diff display is
  already a durable history API.
- Shared-version JSON includes installation choices, Modrinth version references, an optional
  base pack, and external file references. Native sharing also supports a config bundle. This is
  a different input format from a `.mrpack` index; adapt both to the same server apply behavior.

These observations describe this checkout. They do not establish Modrinth's private backend design.

## Implementation sections

1. Finish resource usage reporting using total host capacity.
2. Unify Core's inherited-content records and apply behavior for packs and instance versions.
   Include installation choices, mods/datapacks, allowed local additions, repair/unlink semantics,
   excluded-client content visibility, and typed responses. Config-management expansion and
   inherited-content disabling are deferred. Prove one initial install and one update.
3. Connect instance pushes to durable linked-server targets, supporting several servers,
   offline reconciliation, authorization, and desired/installed status.
4. Design and implement From instance creation, Worlds discovery, public/private visibility,
   and server controls, following the agreed ownership boundary.
5. Design history views around real version/apply data. Defer full Git branches and merge workflows.

## Open product choices

- Full history for all local instances, branches, and merging remain deferred.

## Decided 2026-10-09

- Push buttons: the primary button is "Push update", which never restarts; running servers apply it
  when they next stop. The secondary outline button is "Push and restart", which restarts every
  linked server that is running. No Restart Server preference.
- Anyone with access to an instance who has it installed can create a linked server from it on a
  Core they control. Creating a server on someone else's Core is deferred.
- Players are not prompted for versions that only change server content. Their clients stay on
  their current version and jump straight to the latest one when a version changes client content.

## Clarification ledger

2026-10-04, Ilai's follow-up: direct client-to-Core delivery and database location metadata are
the sharing architecture. Server-only updates have no unrelated-Core storage fallback. This
supersedes the agent's broader storage recommendation above. Applying only while stopped is
accepted; restart is an explicit push choice with a later configurable default.

2026-10-04, next clarification: direct pack upload and shared-storage consumption are distinct
delivery paths. Mods and datapacks are in scope; forced resource packs, new config management,
and disabling inherited shared content are excluded. Unknown custom mods install. Known client-only
content is excluded from installation but remains listed. Apply the latest full version only.
Failure handling is delegated to the agent; Tasks/Notifications integration is explicitly requested.
