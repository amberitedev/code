-- The manifest of the share version a member's instance last applied, so its next update knows
-- which configs left the share without the backend keeping old versions.
CREATE TABLE shared_instance_applied_files (
	instance_id TEXT NOT NULL,
	version INTEGER NOT NULL,
	files TEXT NOT NULL,

	PRIMARY KEY (instance_id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);
