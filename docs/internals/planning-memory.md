# Amberite planning memory

Last updated: 2026-09-24. Source: Ilai's ongoing planning conversation in this thread.

This is a planning record, not an implementation authorization or a description of completed code.
Read this before preparing an implementation handoff. Do not treat every idea below as approved.

Current handoff: [goals and decisions](account-sharing-implementation-plan.md).
Ilai requested deletion of the separate implementation-thread prompt on 2026-09-24.
Implementation has not started. The backend runtime remains unapproved. The client audit is complete;
Ilai subsequently chose an upstream-behavior client baseline, as recorded below. On 2026-09-24 he
requested that the plan explain goals and decisions rather than prescribe implementation steps.

## How to maintain this memory

- Update the current decision sections when Ilai changes a decision.
- Preserve the previous decision in the chronological answer ledger, explicitly marked superseded.
- Distinguish **accepted**, **proposed**, **deferred**, **observed**, and **open**. Agent recommendations
  are not user decisions. Silence is not approval.
- Preserve concrete reasons, examples, direct answers, and unresolved ambiguities. Do not repeatedly
  compress this document into a shorter summary that drops these details.
- Add dates and sources to measurements and provider claims. Recheck provider terms before deployment.
- Keep an entry point here even if supporting research eventually moves into linked files.
- Explicit later user instructions override older planning. This file does not override system,
  developer, or repository instructions. Surface conflicts rather than silently choosing a design.
- This record captures the conversation available to the agent. It is not a verbatim archive of all
  earlier chats. Do not claim missing transcripts have been preserved.
- Do not implement from this memory alone. Prepare a bounded plan and obtain Ilai's approval.

## Current milestone: account and sharing separation

**Accepted.** Recreate the account/social and private instance-sharing behavior the Modrinth client
expects, on our services. Keep the fork close to upstream. This milestone does not require a Minecraft
server, a connection from an instance to a Minecraft server, or new server-management UX.

The user's primary acceptance workflow is:

1. User A adds User B as a friend and shares an existing instance.
2. B accepts the shared instance and installs it.
3. A changes the instance and pushes an update.
4. B downloads and installs that update through the existing Modrinth experience.

Additional tests are now **accepted** by Ilai:

- Account/session lifecycle: distinct users, account switching, app restart, correct user-specific state.
- Owner-offline install/update after completed upload, including interrupted download and retry.

"No server" here means no Minecraft server in the product workflow. Hosted account/sharing APIs and
storage services are still needed. Neither participant should need to own a Core to share normally.

**Next milestone:** server management and a full sharing workflow involving a hosted Minecraft server.
**Afterward:** introduce Amberite-specific integrations/UI incrementally, with Ilai involved in UX.

Do not expand milestone one to cover private-Core settings, incentives, installer UX, server pairing,
PRs/branches, every custom-content format, or peer-to-peer capacity fallback. Placement infrastructure
scope still needs a bounded implementation plan; its eventual behavior is not all mandatory for v1.

## Language and boundaries

- **Central backend:** provider-hosted accounts, sessions, friends, and related account functionality.
- **Sharing backend:** shared-instance metadata, versions, access, storage locations, contributor
  availability, and coordination/processing better performed centrally than on clients.
- These are logical responsibilities. Ilai is interested in combining them. One deployment with
  separate modules is an agent recommendation, not a finalized provider/deployment decision.
- **Core / Copal:** user-hosted Minecraft server-management product. We do not pay to host users' Cores.
- **Storage service:** separable from Minecraft management. Can run alongside a Core or by itself.
- **NAS:** Ilai's separately registered storage/overflow service. Not a Core and not a permanent
  backup archive. It may run the same storage-server component.
- **Shared version / push an update:** preferred wording. Avoid public-project "publishing" language
  when discussing private sharing. Existing source function names may still say publish.
- Avoid "Amberite identity" in explanations. Say accounts or user system.
- Earlier Core/Node terminology is contested: Ilai described a Core as the collection of Nodes;
  older docs describe Core as the process on each Node. Do not silently settle this during sharing work.

## Compatibility and hosting direction

**Accepted intent:** preserve Modrinth account/social/sharing API contracts and observable behavior
where practical. Keep the existing API client as the app-facing boundary. It may coordinate services,
route direct transfers, and normalize responses without forcing new UI flows.

**Latest account-direction correction:** reproduce Modrinth's account behavior first, keeping its
functionality. Minecraft-first adjustments and redesign of the existing Amberite-specific account
model are later. Earlier Minecraft-first sign-in requirements must not silently shape milestone one.
Matching behavior does not mean copying Modrinth's production users, passwords, OAuth client secrets,
or privileged service credentials. Inventory provider configuration separately from portable code.

- Public mods/projects/versions/downloads remain on Modrinth. Do not rebuild its public content platform.
- Port only relevant public Labrinth account/social code, not all of Labrinth.
- Modrinth's sharing and Hosting internals are private. Public client contracts are evidence of what
  replacements must do, not proof of private implementation details.
- Production must be real replacement services, not the research mock/proxy.
- **Updated 2026-09-24:** Ilai chose restoration of `apps/app-frontend` and `apps/app` to upstream
  behavior on a new branch, keeping dev-environment support and the API-client integration boundary.
  Other client changes, including ad cleanup, are preserved for later restoration. This supersedes
  the earlier no-client-reset planning constraint, but does not authorize loss of uncommitted work
  or rewriting main. After the baseline is established, recreate backend functionality with minimal
  client changes; Amberite-specific UI/UX comes afterward.
- User reports that account work was ported into Labrinth. Earlier agent inspection found active
  Convex adapters. Reconcile actual code/branch history before implementation; do not let this disputed
  observation redefine the intended architecture or repeatedly argue it in product planning.
- One TS API client exists, but native Rust sharing code also performs HTTP. Compatibility work must
  account for native transport/auth/upload checks; a base-URL change is not sufficient by itself.

**Proposed hosting:** Cloudflare Workers + D1 + hibernating Durable Objects for lightweight hosted
APIs/metadata/presence. Cloud Run + managed Postgres remains an alternative if retaining Rust/SQL
outweighs adaptation. Neither stack is a finalized implementation approval.

Ilai wants sustainable free-tier operation for an illustrative 1,000-5,000 users. Some small usage-based
expense is acceptable; subscription minimums are undesirable. These are not measured workload targets.
He means metadata is inexpensive to operate, not that Postgres software being free makes hosting free.

## Storage, versions, and recovery

- **Accepted:** two network replicas initially because they are cheap and simple. This is not a
  permanent requirement for an elaborate redundancy system.
- **Accepted:** owner clients retain recovery snapshots. If every network copy is lost, restore
  automatically when a client holding recovery data becomes available. No routine confirmation.
- Recovery is possible only while an intact local snapshot and required keys remain. An ordinary
  mutable installed instance is not a historical snapshot. Never promise recovery after all copies die.
- **Accepted:** latest five shared versions online plus explicitly pinned versions. Older local
  history is retained according to user settings. Local retention budget/default is not settled.
- **Accepted:** NAS holds data when contributor capacity/availability is insufficient. It does not
  permanently retain every version as an extra backup. This supersedes the agent's earlier proposal.
- **Accepted:** recipients may download once a correct complete copy is available; replication need
  not block them. Metadata visibility alone must not falsely imply bytes are retrievable.
- **Accepted engineering direction:** compression, reuse of unchanged content, integrity checks,
  safe copy-verify-retire repair, and background transfers. Exact algorithms are delegated to the agent.
- **Accepted:** encrypt contributed content. Key management/recovery design is open engineering work.
- **Deferred:** erasure coding. It was discussed as a possible storage optimization, not selected.
- Preserve currently supported Modrinth sharing content first. KubeJS/FancyMenu and other custom
  content support is a later extension; no lossless "turn every asset into text" assumption.

## Placement and contributor behavior

- Start with a simple event-driven placement/repair algorithm in the sharing backend.
- Observe availability, successful transfers, storage capacity, and resource budgets. Track enough
  operational history to improve placement later. Do not reassign healthy data for tiny score changes.
- Prefer the sharer's own eligible Core when available. Ilai explicitly says this matters to placement,
  even though the UI/integration around owning a Core comes later. Owning a Core is not required.
- Storage contribution and Minecraft management can fail independently. Report sharing diagnostics
  without preventing unrelated Core features. Fixing sharing connectivity is optional for Core use.
- Integrated desktop Cores do not contribute to the general storage pool in this phase.
- Contribution is expected/enabled by default, without a contribution-choice screen in setup.
  Later settings permit opting out/changing limits. Communicating the behavior is still needed.
- Use fair finite background budgets. Exact values are **not approved**. Agent suggested 5 GB storage,
  20 GB/month combined traffic, 2 Mbps aggregate background speed; these remain proposals.
- User-requested install/update/play transfers get sensible priority; do not redesign inherited update UX.
- Opt-out stops using the server for community contribution; it does not disconnect or disable Core.
  Collect/send necessary state and arrange replacement if needed. No end-of-month obligation.
- **Deferred:** a named "private Core" feature, private-Core UX, contribution incentives/restrictions.
  Do not implement deceptive or fabricated penalties; any actual incentive requires a later decision.

## Capacity exhaustion and direct sharing

Existing shared content must not be harmed to accept new content. Clean up genuinely unused or
expired data and avoid duplicate storage. NAS is additional capacity, not an unlimited-storage guarantee.
Ilai mentioned around 20 GB as plausible extra space but did not set a firm NAS allocation.

**Deferred feature:** direct owner-client to recipient-client sharing when hosted capacity is
unavailable. Both must be online, with UI explaining that direct sharing/owner availability is required.
Ilai called this "offline sharing"; it means without hosted storage, not without network access.

He also proposed routing some large content directly and possible resource-pack exclusions. These
are later ideas, not authorization to remove existing Modrinth resource-pack behavior in milestone one.
Prompts asking contributors to increase capacity are deferred.

## Connectivity and setup

- Tunnel/relay transport is still open research. Do not assume every Core needs a tunnel or that a
  public endpoint means unauthenticated access to private files.
- User-owned Cloudflare Tunnel was suggested. It is not an unrestricted free bulk-file relay.
- **Accepted later installer behavior:** ask whether to set up firewall rules automatically, explain
  the changes, and provide diagnostics. No automatic router changes in the intended setup.
- Installation/firewall/tunnel onboarding is outside the present account/sharing workflow milestone.
- Mandatory contributor-IP hiding is not decided; the user deferred private-Core questions.

Research checked 2026-09-23:

- Cloudflare public Tunnel routes go through its reverse proxy. Large-file restrictions apply; ordinary
  non-HTTP routes need client-side software. Source:
  https://developers.cloudflare.com/tunnel/concepts/routing/
- Iroh attempts direct encrypted connections and uses relays when necessary. Candidate for Rust-native
  connectivity, not an approved dependency. https://docs.iroh.computer/deployment/security-privacy
- Iroh public relays are intended for development/testing, not production. Self-hosted relays require
  reachable infrastructure and bandwidth. https://docs.iroh.computer/add-a-relay
- Iroh listed Community as rate-limited/no uptime guarantee; Pro $19/month including 100 GB relay
  egress at research time. Recheck before use. https://www.iroh.computer/pricing
- Tailscale private networks do not automatically admit unrelated recipients. Funnel is public but
  bandwidth-limited. https://tailscale.com/docs/features/tailscale-funnel
- A streaming Worker upload gateway has no separate Workers bandwidth fee at research time, but is
  not durable buffering; limits, retry behavior, origin reachability and applicable terms still matter.
  https://developers.cloudflare.com/workers/platform/pricing/

## Operational data and optional analytics

Ilai wants useful contributor/client diagnostics and data for improving the algorithm, and is open
to PostHog. He wants a settings option disabling optional data collection. Content should be encrypted;
he did not request end-to-end encryption that makes collected analytics unreadable to the operator.

Do not interpret this as permission to send secrets, credentials, arbitrary config contents, or all
local files to analytics. Define explicit telemetry fields. Distinguish service-required operational
state from optional product analytics. Protect both in transit and restrict access. Analytics opt-out
must work, and sharing should not depend on PostHog availability. These are engineering safeguards,
not claims that Ilai has approved a detailed telemetry policy or legal terms.

Jev was suggested as a future aid to placement decisions. Valid schema does not ensure correct
placement. A later experiment may compare advisory rankings against a deterministic baseline.
Do not make model calls part of the first reliable sharing path without an explicit new decision.

## Evidence: local footprint study

Measured read-only on 2026-09-23 from
`C:/Users/ilai/AppData/Roaming/ModrinthApp/profiles`. No content was uploaded. This is a selected local
sample, not population statistics or an average Modrinth user. No access to Modrinth private PostHog.

Method: count local mod files; independently Deflate files in config/defaultconfigs/kubejs/scripts.
Broader measurement excludes directory segments exported/logs/cache/caches/.cache/backups, retains
original bytes when compression grows a file, and leaves files over 32 MiB uncompressed. Payload sizes
exclude index/archive/encryption/filesystem overhead. Mod JARs and separate resource/shader packs are
excluded. Config-only measurement selects the 14 extensions in the upstream config-sharing code and
does not apply the broader directory exclusions, so the columns are not strictly nested.

| Instance               | Mod files | Eligible configs compressed MB | Broader customization compressed MB |
| ---------------------- | --------: | -----------------------------: | ----------------------------------: |
| 1.21.1 Client          |       113 |                          0.058 |                               0.071 |
| Aero SMP               |       108 |                          0.040 |                                10.7 |
| Just Create SMP 2      |       105 |                          0.240 |                               0.267 |
| Star Technology        |       176 |                          0.137 |                                3.05 |
| CABIN EDITED           |       270 |                          0.659 |                                19.2 |
| Complex Cobblemon      |        71 |                          0.085 |                                23.4 |
| Create Stranded at Sea |       162 |                          0.293 |                                64.5 |
| All the Mods 10        |       481 |                          0.599 |                               191.2 |

Large assets explain the spread: Aero FancyMenu panoramas; CABIN slideshows; Stranded at Sea music
and images; ATM10 KubeJS quest images. ATM10's KubeJS folder was 175.6 MB raw / 172.1 MB compressed.
Separate resource/shader data in the sample was approximately 1-111 MB per instance before further
classification. Known Modrinth content can be referenced; custom/unrecognized content must be counted.

Illustrative history models, NOT observed historical deltas:

| First compressed snapshot / new data per update | 5 versions, 2 replicas | 15 versions, 2 replicas | 20 versions, 2 replicas |
| ----------------------------------------------- | ---------------------: | ----------------------: | ----------------------: |
| 1 MB / 0.05 MB                                  |                 2.4 MB |                  3.4 MB |                  3.9 MB |
| 20 MB / 0.5 MB                                  |                  44 MB |                   54 MB |                   59 MB |
| 200 MB / 5 MB                                   |                 440 MB |                  540 MB |                  590 MB |

Formula: 2 * (first snapshot + (version count - 1) * new unique data). Requires reuse of unchanged
content, not storing a fresh opaque archive for each version. Independent full snapshots cost more.
One decimal GB would hold about 416 / 22 / 2 instances respectively in the five-version scenarios.

Network examples: 20 MB * 100 downloads = 2 GB; 200 MB * 100 = 20 GB. A 1 KB/minute payload is
43.2 MB/30 days before overhead. Continuous 2 Mbps is 648 GB/30 days. A router does not bill traffic;
ISP/server-provider plans determine metering, and upload saturation can hurt latency even with free
traffic. Measure the future storage process; no actual RAM/CPU benchmark exists yet.

Sources for content research:

- https://kubejs.com/wiki/folder-structure
- https://docs.fancymenu.net/docs/en-US/share-layouts
- https://docs.blamejared.com/1.20/en/tutorial/IntroductionToScripting/WhatAreScripts/
- https://modrinth.com/news/article/shared-instances/
- `packages/api-client/src/modules/shared-instances/types.ts`
- `packages/app-lib/src/api/instance/shared/{mod,publish,client}.rs`
- Upstream research checkout: `C:/Users/ilai/worktrees/modrinth-proxy-lab`, inspected sharing-file
  history at commit 8cdf53f59 dated 2026-09-14. Do not call this proof of the latest live backend.

## Earlier product direction, deferred from this milestone

- Long-term product: Modrinth-like private friend-group launcher and self-hosted server management.
- One instance configuration; local and server installations are physical copies on different machines.
- Multiple hosted worlds/servers may use an instance. Do not invent a second independent content UI.
- One content panel with environment indicators/filters and overrides both ways, including exclude
  from client/server. Enforce compatibility warnings without assuming client means no integrated server.
- World/server panel contains runtime settings, logs, backups, etc. Content may redirect to the parent
  instance's content panel. Exact UI remains for Ilai to review later.
- Latest user direction rejects separate world ownership as needless complexity; older product-model
  ownership statements must not silently overrule it. Permissions already have prior design work; defer
  new role design instead of restarting it in this milestone.
- PRs, branches, collaboration, advanced history and server sync are later additions.
- Earlier recovered discussion was titled "Server Instance UX Design", Aug 20-21 2026,
  thread id `10d80589-c13a-4c9f-bad0-1a3e1a63148f`, in T3 local state. Treat as historical context,
  not stronger authority than the latest answers here.

## Answer ledger and supersessions

These excerpts are from the visible planning thread. They preserve intent; surrounding explanation
is retained above. Add new answers rather than erasing this history.

1. NAS: "it shouldn't be holding backups." Supersedes agent recommendation that NAS retain all
   shared versions. Client recovery snapshots supply that role.
2. Contribution setup: "It is expected and enabled by default. When you set it up, it doesn't even ask."
   Supersedes agent recommendation for a setup contribution toggle. Settings changes remain possible.
3. Scope: "No server, just the existing Modrinth features related to sharing and accounts."
   Supersedes broad first-milestone discussion of hosting/private-Core/setup UX.
4. Separation: "The sharing and core features are separate entities." Sharing failures do not disable
   unrelated Core features. Own-Core preference matters later to placement and performance.
5. Setup: "Set up firewall automatically" is a later setup choice with explanation.
   "We can't touch the router automatically" is the desired product constraint, not a factual claim
   that automatic port mapping is technically impossible.
6. Recovery: "I think we should just have automatic restore." No routine prompt required.
7. History: "I agree with your recommendation for question 2." Accepts five recent online versions
   plus pinned versions and older locally recoverable history from the immediately preceding round.
8. Private Core: "let's ignore private cores and related items. This is a feature for later."
   Do not ask further private-Core questions as blockers for the current milestone.
9. Direct fallback: "Make sure the owner is online to download the update" is an explicitly later
   fallback when hosted storage is unavailable, not the default sharing availability contract.
10. Encryption: "Encrypt content" accepted. Optional collection can be disabled in settings.
    Detailed telemetry fields and policy remain to be designed.
11. Memory: "Write down everything, actually" and keep outdated decisions adjusted. Authorizes these
    planning documentation edits; does not authorize application implementation.
12. Interview style: sections and the whole relevant question frontier, not arbitrary batches of three.
    User delegates ordinary implementation details. Ask product decisions, not facts we can inspect.
13. Additional acceptance tests: "I like those two other tests that you added" accepts the session/
    account lifecycle and owner-offline install/update plus interrupted-transfer tests.
14. Account direction: "Just keeping it exactly as Modrinth is good"; Minecraft-first changes later.
    Preserve account feature parity first rather than extending the earlier custom account design.
15. NAS connectivity: Ilai cannot port-forward with his current router. Direct inbound HTTPS on the
    NAS is not an established option. A tunnel/relay or another reachable storage endpoint is needed.
16. Provider split: investigate using separate free allowances, but user suspects one hosted backend
    may be simpler because shares reference users. No multi-provider deployment approved.
17. Core split: hosting, storage/sharing, and genuinely reusable code were suggested as possible
    sections. Agent must recommend minimal boundaries; no three-service rewrite authorized.
18. Dev first: "We make dev work and then we make other stuff work." Defer tunnels, installation,
    public deployment and setup. Do not block local implementation on the NAS's inbound connectivity.
19. Official reference access: Ilai says installer-provided credentials are required for the official
    sharing server. Use authorized official installations for reference checks; do not assume source
    builds or data-directory overrides establish access. The exact credential mechanism is unverified.
20. One-PC reference testing accepted: "we don't need a second computer. We can test here." Two
    installations/accounts are an option, not a verified installer capability or authorization to
    install/reset anything immediately.
21. Handoff: user requested an implementation plan file and a prompt with verification/subagent
    instructions. This turn writes those documents only; it does not start the implementation.

## Open items and next planning action

- Reconcile account API/current code and choose final hosting/port scope with measured authentication
  constraints. Do not force all historical product decisions into this check.
- Recommend a single hosted deployment for central/sharing modules; confirm in implementation plan.
- Determine minimum real storage integration for first milestone, distinct from full contributor rollout.
- Three acceptance workflows are approved above. Specify test details without reopening their scope.
- Preserve resource-pack behavior unless Ilai explicitly changes first-milestone compatibility scope.
- Defer public tunneling/setup; dev-first sharing does not depend on that choice. Preserve the
  connectivity research for the later deployment/setup milestone.
- Local recovery retention budget, encryption-key recovery, consent wording, abuse controls, and exact
  contributor limits require engineering design or later product decisions, not invented approval.
- No implementation swarm has been launched. User wants subagents eventually, but coordinating a
  fresh implementation thread/tooling is deferred while he investigates T3 capabilities.

### Latest testing and separation findings

- Local upstream `packages/app-lib/src/state/dirs.rs` reads `THESEUS_CONFIG_DIR` for the initial
  settings directory. `apps/app/src/main.rs` installs the single-instance plugin unconditionally.
  This suggests sequential isolated launcher-data sessions on one PC, not guaranteed simultaneous
  stock installations. Browser/WebView state isolation and the installed release must be verified.
  **Later clarification:** this source observation is not proof that official service credentials
  survive or appear in a fresh directory. Ilai requires official installed Modrinth for reference tests.
- Two distinct Modrinth test accounts matter more than two physical computers. Prefer sequential
  isolated sessions; a separate OS user or VM is a fallback. No second computer is currently justified
  for the basic invite/update reference workflow. Cross-network connectivity testing is a later need.
- Infer response contracts from response types/parsers and UI consumers, not request shape alone.
  Source review establishes expected contracts; authorized live observations resolve ambiguous
  backend semantics. Mocks are not evidence of official backend behavior.
- Capture only user-authorized test traffic; keep tokens, cookies, credentials, and private payloads
  out of committed fixtures. Never weaken TLS globally or probe other users' data.
- Proposed modular monolith: one hosted account/sharing deployment and relational database, separate
  internal modules. Split deployment only for a measured quota/runtime reason. Multiple Workers in
  one account do not multiply the account's free request allowance.
- Proposed Core structure: leave Minecraft management in place, add a storage-only executable/module
  that can also run alongside Core, extract only concrete shared protocol/auth/diagnostic utilities.
  Avoid a generic shared-assets framework or broad refactor before the sharing acceptance workflow.

## Known stale documents

`product-model.md`, `glossary.md`, and supplied AGENTS text contain older Convex/Labrinth/social,
Core/Node, permission, ownership and publishing terminology. Preserve useful material, but check
against the statuses above before using it. Their pointers to Convex describe existing code, not the
approved destination architecture. Do not rewrite those documents wholesale without a scoped task.
