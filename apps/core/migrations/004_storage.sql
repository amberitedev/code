-- Storage this Core offers the shared file pool (`store`). On by default.
ALTER TABLE core ADD COLUMN storage_enabled INTEGER NOT NULL DEFAULT 1;
-- Bytes the store may hold. Null: a tenth of the disk's free space at start, between 10 and 20 GiB.
ALTER TABLE core ADD COLUMN storage_cap INTEGER;
-- Bytes the store may serve a day. 50 GiB.
ALTER TABLE core ADD COLUMN storage_egress_cap INTEGER NOT NULL DEFAULT 53687091200;
