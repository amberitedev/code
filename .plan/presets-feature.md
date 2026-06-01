# Plan: Presets (cross-version personal mod / settings / config sets)

A **Preset** is a user-owned, instance-independent set of mods (and later settings/configs)
that you maintain once and re-apply to any pack. Unlike an instance, a preset is not a
playable profile — it is a saved "this is the stuff I always want" definition, organized
per game version (different mods/files per MC version + loader). When you install a pack
from a friend, your active preset(s) are layered on top so your personal extras come along.

This is the launcher-side analogue of the synced-profile idea: synced profiles share a set
within a friend group; presets are *your* personal set across all your packs.

## Naming

"Profiles" already means instances. To avoid collision we use **Preset** in code/UI
(`preset`, `Preset`, `presets`). User-facing label can still be "Profiles" if desired, but
the type/identifier is `preset`.

## Data model

Greenfield. Reuse the manifest shape already used by `profileSnapshots.manifest` in Convex
so apply/diff logic can be shared with the sync pipeline later.

New Convex table `presets`:
- `ownerUserId`
- `name`, `summary?`, `icon?`
- `kind`: 'mods' | 'settings' | 'configs'  (start with 'mods')
- `enabledByDefault`: boolean  (auto-layer on new installs)
- `createdAt`, `updatedAt`
- index `by_owner`

New Convex table `presetVersions` (per game-version slice of a preset):
- `presetId`
- `gameVersion` (e.g. '1.21.1'), `loader` ('fabric' | ... | 'any')
- `manifest`: any  (same shape as `profileSnapshots.manifest`: list of
  { projectId, versionId?, source, side, fileName, hash? })
- `clientOnly`: boolean
- index `by_preset`, `by_preset_version`

`ConvexApiClient` additions (packages/amberite-api/src/convex-api.ts):
- `listMyPresets()`
- `getPreset(presetId)`
- `createPreset({ name, kind, enabledByDefault })`
- `updatePreset(presetId, patch)`
- `deletePreset(presetId)`
- `upsertPresetVersion(presetId, { gameVersion, loader, manifest, clientOnly })`
- `listPresetVersions(presetId)`
Types in convex-types.ts: `PresetInfo`, `PresetVersion`.

## Apply / layering

When installing a pack (create_profile_and_install path), after the pack resolves its
game version + loader, gather the user's `enabledByDefault` presets, pick each preset's
matching `presetVersion` (exact gameVersion+loader, else loader='any', else nearest), and
queue its manifest entries as additional content installs on top of the pack.
- Reuse the existing content-install helpers rather than a new installer.
- Conflicts (preset pins a mod the pack already includes): pack wins unless preset pins an
  explicit versionId; surface a small summary "added N mods from your presets".
- Defer the real diff/apply engine — Convex `sync.ts` already notes the diff/apply pipeline
  is intentionally deferred; presets should plug into that same engine when it lands.

## Settings / configs presets (phase 2)

Same tables, `kind: 'settings' | 'configs'`. Manifest becomes a list of
{ path, content | patch }. Apply writes files into the instance dir after install. Needs a
safe-merge strategy for options.txt / loader config files (key-level merge, not overwrite).

## UI — lives on the (not-yet-existing) Core page

PREREQUISITE: there is currently **no Core page in the frontend** (friendgroups exist only
in Convex + ConvexApiClient). The Presets UI is intended to sit as a tab on that Core page,
next to friend groups. Until the Core page exists, the Presets manager has no host.

Proposed UI once the Core page exists:
- `Presets` tab: list of presets (card per preset: icon, name, kind, enabled toggle).
- Preset detail: per-version sections (1.21.1 / Fabric ...), each a content list reusing the
  existing content-tab list components; add/remove mods via the existing search/install modal.
- "Enabled by default" toggle controls auto-layering on new installs.

## Build order

1. Convex schema (`presets`, `presetVersions`) + functions (`presets.ts`).
2. `ConvexApiClient` methods + types.
3. A `usePresets` composable in the app (needs a `useConvexClient()` first — also missing).
4. Layering hook in the install path.
5. UI on the Core page (after the Core page exists).

Status: planned. Blocked on (a) a frontend Convex client composable and (b) the Core page
shell. Permissions feature (separate plan) is the better first build since its backend
backbone already exists.
