-- General scheduled tasks table: restart, announce, command execution, plus backup (unifies with backup_schedules later).
CREATE TABLE scheduled_tasks (
	id TEXT PRIMARY KEY,
	instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
	task_type TEXT NOT NULL CHECK(task_type IN ('backup','restart','command','announce')),
	cron TEXT NOT NULL,
	enabled INTEGER NOT NULL DEFAULT 1,
	payload TEXT,
	created_at TEXT NOT NULL,
	updated_at TEXT NOT NULL,
	last_run_at TEXT
);

CREATE INDEX idx_scheduled_tasks_instance ON scheduled_tasks(instance_id);
CREATE INDEX idx_scheduled_tasks_enabled ON scheduled_tasks(enabled);
