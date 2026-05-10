-- Migration 006: Back-fill NULL sentinel values for rows created before migration 003.
--
-- Migration 003 added columns with ALTER TABLE ADD COLUMN but without DEFAULT
-- values, leaving NULL in those columns for any rows that pre-date it. The Rust
-- InstanceRow struct decodes these as non-optional types and would return a
-- ColumnDecode error at runtime (MIGS-01).
--
-- We set safe sentinel values so existing rows decode successfully. New rows are
-- always written with real values by the application.

UPDATE instances SET game_version = 'unknown' WHERE game_version IS NULL;
UPDATE instances SET loader       = 'vanilla' WHERE loader IS NULL;
UPDATE instances SET port         = 25565     WHERE port IS NULL;
UPDATE instances SET memory_min   = 1024      WHERE memory_min IS NULL;
UPDATE instances SET memory_max   = 4096      WHERE memory_max IS NULL;
UPDATE instances SET data_dir     = ''        WHERE data_dir IS NULL;
