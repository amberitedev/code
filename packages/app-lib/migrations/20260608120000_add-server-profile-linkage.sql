ALTER TABLE profiles
ADD COLUMN core_instance_id TEXT NULL;

ALTER TABLE profiles
ADD COLUMN server_manifest_json JSONB NULL;
