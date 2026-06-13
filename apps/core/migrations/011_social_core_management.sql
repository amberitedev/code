CREATE TABLE IF NOT EXISTS core_metadata (
	id INTEGER PRIMARY KEY CHECK (id = 1),
	name TEXT NOT NULL DEFAULT 'Copal',
	description TEXT,
	banner TEXT,
	subdomain TEXT,
	setup_mode TEXT NOT NULL DEFAULT 'remote',
	run_mode TEXT NOT NULL DEFAULT 'manual',
	updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS core_members (
	user_id TEXT PRIMARY KEY,
	display_name TEXT,
	role TEXT NOT NULL DEFAULT 'member',
	permission_preset TEXT NOT NULL DEFAULT 'member',
	custom_permissions TEXT,
	status TEXT NOT NULL DEFAULT 'active',
	joined_at TEXT NOT NULL,
	updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS core_group_bans (
	user_id TEXT PRIMARY KEY,
	reason TEXT,
	banned_by TEXT,
	banned_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_profiles (
	id TEXT PRIMARY KEY,
	client_profile_id TEXT,
	core_instance_id TEXT,
	name TEXT NOT NULL,
	game_version TEXT,
	loader TEXT,
	sync_enabled INTEGER NOT NULL DEFAULT 1,
	created_at TEXT NOT NULL,
	updated_at TEXT NOT NULL,
	last_snapshot_at TEXT
);

CREATE TABLE IF NOT EXISTS sync_snapshots (
	id TEXT PRIMARY KEY,
	profile_id TEXT NOT NULL REFERENCES sync_profiles(id) ON DELETE CASCADE,
	author_user_id TEXT NOT NULL,
	manifest_json TEXT NOT NULL,
	client_only_json TEXT,
	server_manifest_json TEXT,
	notes TEXT,
	created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_events (
	id TEXT PRIMARY KEY,
	profile_id TEXT NOT NULL REFERENCES sync_profiles(id) ON DELETE CASCADE,
	snapshot_id TEXT REFERENCES sync_snapshots(id) ON DELETE SET NULL,
	status TEXT NOT NULL DEFAULT 'planned',
	diff_json TEXT,
	message TEXT,
	created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_core_members_role ON core_members(role);
CREATE INDEX IF NOT EXISTS idx_sync_profiles_core_instance ON sync_profiles(core_instance_id);
CREATE INDEX IF NOT EXISTS idx_sync_snapshots_profile ON sync_snapshots(profile_id, created_at);
CREATE INDEX IF NOT EXISTS idx_sync_events_profile ON sync_events(profile_id, created_at);
