-- The owner's local history of each share it pushed (sharing_client::history). The files of an
-- entry are retained in the content store under owner kind 'share_history'.
CREATE TABLE share_history (
	share_id TEXT NOT NULL,
	version INTEGER NOT NULL,
	entry TEXT NOT NULL,

	PRIMARY KEY (share_id, version)
);

-- The manifest of the share version a member's instance last applied, so its next update knows
-- which configs left the share without the backend keeping old versions.
CREATE TABLE shared_instance_applied_files (
	instance_id TEXT NOT NULL,
	version INTEGER NOT NULL,
	files TEXT NOT NULL,

	PRIMARY KEY (instance_id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);
