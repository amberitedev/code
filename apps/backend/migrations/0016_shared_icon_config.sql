-- The icon editor settings a share's icon was made from, as the App sends them, so members can
-- edit a copy. NULL for an uploaded image.
ALTER TABLE shared_instances ADD COLUMN icon_config TEXT;
