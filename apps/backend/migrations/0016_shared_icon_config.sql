-- An icon editor icon is kept as its settings, which clients draw themselves. NULL for an
-- uploaded image.
ALTER TABLE shared_instances ADD COLUMN icon_config TEXT;
-- Uploaded images live in R2 under the share (`iconKey`), so rows hold only their URL.
DROP INDEX shared_instances_icon;
ALTER TABLE shared_instances DROP COLUMN icon_data;
ALTER TABLE shared_instances DROP COLUMN icon_type;
