-- Adds cumulative uptime tracking for instances.
ALTER TABLE instances ADD COLUMN total_uptime_seconds INTEGER NOT NULL DEFAULT 0;
