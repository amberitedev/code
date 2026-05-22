ALTER TABLE sync_profiles ADD COLUMN current_snapshot_id TEXT;

ALTER TABLE sync_snapshots ADD COLUMN archive_path TEXT;
ALTER TABLE sync_snapshots ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;

ALTER TABLE sync_events ADD COLUMN applied_at TEXT;

CREATE INDEX IF NOT EXISTS idx_sync_profiles_current_snapshot ON sync_profiles(current_snapshot_id);
CREATE INDEX IF NOT EXISTS idx_sync_snapshots_archive ON sync_snapshots(archived);
