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
- A ready version adds `pool` (holders and a fetch ticket) for members, unless it has no pool
  files (`root` is the empty root). An unconfirmed one adds `store` (home Core and a store ticket) for the owner. The owner also gets `reports`.
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
- `/v1/servers/{core}/{server}/players` (`GET`, `POST`, `DELETE`) and `/invites[/{id}]`: the
  players of a Core server, the people it is shared with, in the shapes of `users` and `invites`.
  They read the share the server follows without being members, and accept and leave as members
  do. A write names the share (`instance_id`), the server's `name` and `port`, and `outside`, the
  Core's `net.outside` (`{ ip, port }`, or null, which keeps the last one known). The App's
  Play tab reaches these through `sharedinstances.players_v1`. Players read the share only while
  the server's Core owner owns it or is a joined member; a member who leaves or is kicked takes
  their servers, players, and links with them. Players get 403 `members_only` on `GET users`, and
  a server's link previews as the server, invited by its Core's owner, with no `instance_users`.
- `DELETE /core/players` (Core key, body `server_id` and `share_id`): the server was deleted or
  stopped following an instance, so its players and links go. A `share_id` limits this to while
  the server still follows that share, so a late call keeps the players of a share it follows
  since; `null`, for a deleted server, drops them whatever it follows.
- `GET /v1/instances/{id}/access`, owner only: `users`, the members and the players of every server
  following the share, each with `via` saying how they got access; and `links`, every server's
  live link, which the owner revokes with `DELETE invites/{id}`. For the instance's Share tab.
- `POST /v1/instances/{id}/versions/{v}/reports`: a member reports broken files.
- `GET` and `PATCH /v1/storage`: the account's pool usage and kept-versions setting.
- `GET /v1/icons/{id}`: instance icons, stored in D1 instead of a CDN.

## Fields that are always empty

- `backtrack_version` is never set.
- `last_played` on members is always null.
- `linked_server` on `GET /v1/instances/{id}` is the server a player came through, and null for
  members. Its `domain` is the server's last known `outside` `ip:port`; without one, the host of
  the Core's `public_address`, then of its reported `address`, with the server's port. Its
  `region` is empty and `online_status` null.
- `GET /v1/instances/{id}/server-icon` is always 404: the backend keeps no server icons.

## Missing

- No mrpack export: `GET /v1/instances/{id}/versions/{v}/mrpack` does not exist (AMB-51).
- Deleting one file through moderation quarantines the whole instance, since the backend cannot
  read the files.
