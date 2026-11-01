# Sharing API

The shared-instances API in `apps/backend/src/sharing` has no upstream server to port: labrinth
removed it. Its spec is the client, the response structs in
`packages/app-lib/src/api/instance/shared/client.rs` and the types in
`packages/api-client/src/modules/shared-instances`. This page lists every place the backend
differs from what that client was written for, on purpose.

## Files live in the pool

- A version carries `root` (R), and each `external_files` entry carries `sha256`, `object`,
  `object_size`, and `key` for non-content files. `url` is `pool:<object>`, which nothing fetches.
  See `pool.md`, "Protocol".
- A ready version adds `pool` (holders and a fetch ticket) for members. An unconfirmed one adds
  `store` (home Core and a store ticket) for the owner. The owner also gets `reports`.
- The push may name `server_core`, the owner's Core running a server of the instance, where the
  share is then homed.
- The unauthenticated invite preview (`GET /v1/invites/{id}?version=true`) returns the latest
  ready version without `key`, `pool`, or `store`, and no `version` while none is ready or the
  instance is quarantined.
- A member reading a version that is not ready gets 404, which the App reads as "never ready".

## Routes the upstream client did not have

- `GET /v1/instances/{id}/servers`: servers other people host for the instance
  (`apps/backend/src/cores/servers.ts`).
- `GET /v1/instances/{id}/history`: kept versions for the version history.
- `POST /v1/instances/{id}/versions/{v}/reports`: a member reports broken files.
- `GET` and `PATCH /v1/storage`: the account's pool usage and kept-versions setting.
- `GET /v1/icons/{id}`: instance icons, stored in D1 instead of a CDN.

## Fields that are always empty

- `backtrack_version` is never set.
- `last_played` on members is always null.
- `linked_server` on `GET /v1/instances/{id}` is null, and `GET /v1/instances/{id}/server-icon`
  is always 404, until shares know their linked server.

## Missing

- No mrpack export: `GET /v1/instances/{id}/versions/{v}/mrpack` does not exist (AMB-51).
- Deleting one file through moderation quarantines the whole instance, since the backend cannot
  read the files.
