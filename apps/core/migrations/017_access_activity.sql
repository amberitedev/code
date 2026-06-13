CREATE TABLE IF NOT EXISTS instance_members (
	instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
	user_id TEXT NOT NULL,
	display_name TEXT,
	role TEXT NOT NULL DEFAULT 'member',
	permission_preset TEXT NOT NULL DEFAULT 'member',
	custom_permissions TEXT,
	status TEXT NOT NULL DEFAULT 'active',
	created_at TEXT NOT NULL,
	updated_at TEXT NOT NULL,
	PRIMARY KEY (instance_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_instance_members_user ON instance_members(user_id);
CREATE INDEX IF NOT EXISTS idx_instance_members_instance ON instance_members(instance_id);

CREATE TABLE IF NOT EXISTS activity_log (
	id TEXT PRIMARY KEY,
	actor_user_id TEXT NOT NULL,
	action TEXT NOT NULL,
	instance_id TEXT,
	target_user_id TEXT,
	metadata_json TEXT,
	created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_activity_log_created ON activity_log(created_at);
CREATE INDEX IF NOT EXISTS idx_activity_log_instance_created ON activity_log(instance_id, created_at);
CREATE INDEX IF NOT EXISTS idx_activity_log_actor_created ON activity_log(actor_user_id, created_at);
