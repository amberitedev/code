-- Migration 003: Add missing columns to instances table.
-- Migration 002 used CREATE TABLE IF NOT EXISTS which silently skipped because
-- migration 001 already created the table. This adds the missing columns safely.

ALTER TABLE instances ADD COLUMN game_version TEXT;
ALTER TABLE instances ADD COLUMN loader TEXT;
ALTER TABLE instances ADD COLUMN port INTEGER;
ALTER TABLE instances ADD COLUMN memory_min INTEGER;
ALTER TABLE instances ADD COLUMN memory_max INTEGER;
ALTER TABLE instances ADD COLUMN status TEXT DEFAULT 'offline';
ALTER TABLE instances ADD COLUMN data_dir TEXT;
