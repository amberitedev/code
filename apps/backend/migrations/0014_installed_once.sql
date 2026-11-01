-- Set the first time someone other than the owner joins a share, and never cleared, so the
-- share-review prompts stay once a friend has installed it even after they leave.
ALTER TABLE shared_instances ADD COLUMN installed_once INTEGER NOT NULL DEFAULT 0;
