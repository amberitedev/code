-- Migration 005: Add missing columns to instances table omitted in migration 003.
--
-- Migration 002 used CREATE TABLE IF NOT EXISTS which was silently skipped because
-- migration 001 had already created the instances table. Migration 003 tried to fix
-- this with ALTER TABLE but missed three columns: loader_version, java_version,
-- updated_at. This migration adds them.
--
-- Existing rows (production DBs) will have:
--   loader_version  = NULL   (correct — it's nullable)
--   java_version    = NULL   (correct — it's nullable)
--   updated_at      = epoch  (safe sentinel; parse succeeds as DateTime<Utc>)

ALTER TABLE instances ADD COLUMN loader_version TEXT;
ALTER TABLE instances ADD COLUMN java_version INTEGER;
ALTER TABLE instances ADD COLUMN updated_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00+00:00';
