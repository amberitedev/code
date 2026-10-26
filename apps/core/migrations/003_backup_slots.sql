-- Backups take the App's slot model: `keep` is the automatic slots, 0 turning scheduled backups
-- off, and `interval_hours` always holds an interval. `skipped_at` is when the schedule last came
-- due with nothing new to back up, so the next due time Core reports is one it keeps.
ALTER TABLE backup_schedules ADD COLUMN skipped_at INTEGER;
UPDATE backup_schedules SET keep = 0, interval_hours = 24 WHERE interval_hours = 0;
-- `backups::SLOTS`: automatic and manual slots together.
UPDATE backup_schedules SET keep = 15 WHERE keep > 15;
