CREATE TABLE IF NOT EXISTS core_roles (
	id TEXT PRIMARY KEY,
	name TEXT NOT NULL COLLATE NOCASE UNIQUE,
	description TEXT NOT NULL DEFAULT '',
	icon TEXT NOT NULL,
	grants_json TEXT NOT NULL,
	retired_at TEXT,
	created_at TEXT NOT NULL,
	updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS core_role_settings (
	id INTEGER PRIMARY KEY CHECK (id = 1),
	require_invite_approval INTEGER NOT NULL DEFAULT 1,
	updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS core_invitations (
	id TEXT PRIMARY KEY,
	invitee_user_id TEXT NOT NULL,
	invitee_display_name TEXT,
	role_id TEXT NOT NULL,
	role_snapshot_json TEXT NOT NULL,
	inviter_user_id TEXT NOT NULL,
	status TEXT NOT NULL,
	created_at TEXT NOT NULL,
	updated_at TEXT NOT NULL,
	expires_at TEXT NOT NULL,
	responded_at TEXT,
	reviewed_by_user_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_core_invitations_invitee_status ON core_invitations(invitee_user_id, status);
CREATE INDEX IF NOT EXISTS idx_core_invitations_status ON core_invitations(status);

ALTER TABLE core_members ADD COLUMN role_id TEXT;
ALTER TABLE core_members ADD COLUMN role_snapshot_json TEXT;
ALTER TABLE core_members ADD COLUMN needs_role_reassignment_at TEXT;

INSERT OR IGNORE INTO core_roles (id, name, description, icon, grants_json, created_at, updated_at)
VALUES
	('role-admin', 'Admin', 'Manages members and Core settings.', 'shield', '["invite-members","remove-members","ban-members","manage-roles","edit-member-roles","approve-invites","manage-instances","start-stop-instances","restart-instances","manage-mods","manage-worlds","manage-files","manage-backups","manage-network","read-console","write-console","edit-settings","view-activity","export-data","dangerous-actions"]', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
	('role-member', 'Member', 'Uses shared Core access.', 'user', '["start-stop-instances","restart-instances","read-console","view-activity"]', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO core_role_settings (id, require_invite_approval, updated_at)
VALUES (1, 1, CURRENT_TIMESTAMP);

UPDATE core_members
SET role_id = CASE role
	WHEN 'admin' THEN 'role-admin'
	WHEN 'member' THEN 'role-member'
	ELSE NULL
END
WHERE role_id IS NULL;
