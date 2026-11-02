-- A server-first share: the share of a Core server that follows no instance, written by that Core
-- with its key (docs/internals/plan.md, "Shares, members, and players"). Its owner is the Core's
-- owner account. Null on every share an App writes.
ALTER TABLE shared_instances ADD COLUMN core_id TEXT;
ALTER TABLE shared_instances ADD COLUMN server_id TEXT;
-- One share per server.
CREATE UNIQUE INDEX shared_instances_server ON shared_instances(core_id, server_id)
  WHERE core_id IS NOT NULL;
