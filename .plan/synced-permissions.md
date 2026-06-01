# Plan: Permissions system for synced pages

Per-user permissions that drive what a member can see/do on a synced profile (and later the
whole Core page). Based on permissions the UI either **grays out** an action ("you can't do
this") or **hides** a section entirely. Roles like owner / admin / member, where e.g. a
member can edit client-side mods but cannot touch server mods or server settings.

## Backend backbone — ALREADY EXISTS

The Convex backend already stores everything we need (see exploration report):
- `friendGroupMembers.role`: 'owner' | 'admin' | 'member'  (convex/schema.ts:40)
- `friendGroupMembers.permissionPreset`: string  (free-form, currently only 'owner' is set)
- `friendGroupMembers.customPermissions`: any  (free-form, unused)
- RBAC helpers in convex/_socialRules.ts (`roleRank`, `requireFriendGroupRole`).
- `ConvexApiClient.updateMemberRole(groupId, userId, role, permissionPreset?, customPermissions?)`.
- Types: `FriendGroupMember`, `FriendGroupSummary` carry `role` + `permissionPreset`.

So roles are enforced server-side for group mutations. What is missing is a **shared
granular permission vocabulary** and **frontend gating** of the synced UI.

## Frontend model (this build)

New module: apps/app-frontend/src/pages/instance/synced/use-synced-permissions.ts

- `SyncedPermission` union (granular capabilities):
  server:view, server:power, server:content, server:files, server:backups, server:settings,
  client:view, client:content, client:settings, instance:settings, members:manage
- `SyncedPermissionPreset`: 'owner' | 'admin' | 'member' | 'client-only' | 'viewer'
  (preset names line up with `friendGroupMembers.permissionPreset`).
- `SYNCED_PERMISSION_PRESETS`: Record<preset, SyncedPermission[]> mapping.
  - owner/admin: all
  - member: server:view, client:view, client:content, client:settings, instance:settings
  - client-only: client:view, client:content, client:settings
  - viewer: server:view, client:view
- `provideSyncedPermissions(source)` / `useSyncedPermissions()`:
  reactive `{ has(p), permissions }`. Defaults to 'owner' (all) so a solo user is unchanged.
- `useSyncedSideTabs()`: returns `{ side, tabs }` where `tabs` = SYNCED_SIDE_TABS filtered by
  `server:view` / `client:view`, and auto-corrects the active side if its tab is hidden.

## Integration (this build)

- SyncedIndex provides the permission context (a `Ref<SyncedPermissionPreset>`), defaulting
  to 'owner'. Server power buttons gated by `server:power`. This ref is the seam where real
  `friendGroupMembers.role/permissionPreset` data plugs in once the synced profile is linked
  to a friend group.
- Each synced wrapper (Content/Files/Backups/Settings/Overview) swaps
  `useSyncedSide()`/`SYNCED_SIDE_TABS` for `useSyncedSideTabs()` so disallowed sides vanish.
- SyncedSettings hides the server card without `server:settings`, client card without
  `client:settings`.
- "View as" preview submenu in the SyncedIndex overflow menu lets you preview the page as
  owner/admin/member/client-only/viewer — clearly a preview tool, and the natural place the
  real role indicator/selector will live. Lets us SEE gating work before group data is wired.

## Later (not this build)

- Wire the permission context to real data: resolve the synced profile's `friendGroupId`
  (Convex `syncedProfiles`), fetch the current user's membership, map role+permissionPreset
  to a `SyncedPermissionPreset`. Needs a frontend Convex client composable (missing).
- Promote the granular `SyncedPermission` vocabulary into Convex `customPermissions` so
  per-user overrides beyond presets are possible, and enforce server-side in sync mutations.
- Reuse the same permission model for the Core page (members management, presets).

Status: frontend model + gating + preview = this build. Real data wiring deferred on the
Convex client composable + group linkage.
