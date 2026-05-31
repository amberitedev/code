-- Adds custom launch tuning fields to instances.
-- `jvm_args`  — extra JVM flags inserted between memory flags and the jar/args file.
-- `server_args` — extra program arguments appended after the default launch args.
-- Both are nullable; NULL means "no overrides" and the default invocation is used.
ALTER TABLE instances ADD COLUMN jvm_args TEXT;
ALTER TABLE instances ADD COLUMN server_args TEXT;
