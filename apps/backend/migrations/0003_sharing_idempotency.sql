ALTER TABLE shared_versions ADD COLUMN request_key TEXT;
ALTER TABLE shared_versions ADD COLUMN request_hash TEXT;
CREATE UNIQUE INDEX shared_versions_request ON shared_versions(instance_id,request_key);
