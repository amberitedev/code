-- Migration 003: Add missing columns to instances table.
-- Migration 002 used CREATE TABLE IF NOT EXISTS which silently skipped because
-- migration 001 already created the table. This adds the missing columns safely.

ALTER TABLE instances ADD COLUMN IF NOT EXISTS game_version TEXT;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS loader TEXT;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS port INTEGER;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS memory_min INTEGER;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS memory_max INTEGER;
ALTER TABLE instances ADD COLUMN IF NOT EXISTS status TEXT DEFAULT 'offline';
ALTER TABLE instances ADD COLUMN IF NOT EXISTS data_dir TEXT;
