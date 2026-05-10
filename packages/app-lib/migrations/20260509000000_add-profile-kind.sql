-- AMBERITE PATCH: Add profile kind (client/server/synced) and Core instance linkage.
ALTER TABLE profiles ADD COLUMN kind TEXT NOT NULL DEFAULT 'client';
ALTER TABLE profiles ADD COLUMN core_instance_id TEXT;
