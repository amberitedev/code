CREATE TABLE settings (
    id INTEGER NOT NULL CHECK (id = 0),

    max_concurrent_downloads INTEGER NOT NULL DEFAULT 10,
    max_concurrent_writes INTEGER NOT NULL DEFAULT 10,

    theme TEXT NOT NULL DEFAULT 'dark',
    default_page TEXT NOT NULL DEFAULT 'home',
    collapsed_navigation INTEGER NOT NULL DEFAULT TRUE,
    advanced_rendering INTEGER NOT NULL DEFAULT TRUE,
    native_decorations INTEGER NOT NULL DEFAULT FALSE,

    telemetry INTEGER NOT NULL DEFAULT TRUE,
    discord_rpc INTEGER NOT NULL DEFAULT TRUE,
    developer_mode INTEGER NOT NULL DEFAULT FALSE,

    extra_launch_args JSONB NOT NULL,
    -- array of (string, string)
    custom_env_vars JSONB NOT NULL,
    mc_memory_max INTEGER NOT NULL DEFAULT 2048,
    mc_force_fullscreen INTEGER NOT NULL DEFAULT FALSE,
    mc_game_resolution_x INTEGER NOT NULL DEFAULT 854,
    mc_game_resolution_y INTEGER NOT NULL DEFAULT 480,

    hide_on_process_start INTEGER NOT NULL DEFAULT FALSE,

    hook_pre_launch TEXT NULL,
    hook_wrapper TEXT NULL,
    hook_post_exit TEXT NULL,

    custom_dir TEXT NULL,
    prev_custom_dir TEXT NULL,
    personalized_ads INTEGER NOT NULL DEFAULT TRUE,
    toggle_sidebar INTEGER NOT NULL DEFAULT FALSE,
    feature_flags JSONB NOT NULL default '{}',
    hide_nametag_skins_page INTEGER NOT NULL DEFAULT 0 CHECK (hide_nametag_skins_page IN (0, 1)),
    skipped_update TEXT NULL,
    pending_update_toast_for_version TEXT NULL,
    auto_download_updates INT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    locale TEXT NOT NULL DEFAULT 'en-US',
    sync_theme_across_devices INTEGER NOT NULL DEFAULT TRUE,
    sync_behavior_across_devices INTEGER NOT NULL DEFAULT TRUE,
    show_files_tab_in_instances INTEGER NOT NULL DEFAULT TRUE CHECK (show_files_tab_in_instances IN (0, 1)),
    show_worlds_tab_in_instances INTEGER NOT NULL DEFAULT TRUE CHECK (show_worlds_tab_in_instances IN (0, 1)),
    show_screenshots_tab_in_instances INTEGER NOT NULL DEFAULT FALSE CHECK (show_screenshots_tab_in_instances IN (0, 1)),
    show_skin_selector_in_sidebar INTEGER NOT NULL DEFAULT TRUE CHECK (show_skin_selector_in_sidebar IN (0, 1)),
    sync_features_across_devices INTEGER NOT NULL DEFAULT TRUE,
    refocus_on_game_close INTEGER NOT NULL DEFAULT FALSE CHECK (refocus_on_game_close IN (0, 1)),
    compact_instance_cards INTEGER NOT NULL DEFAULT FALSE CHECK (compact_instance_cards IN (0, 1)),
    show_play_time INTEGER NOT NULL DEFAULT TRUE CHECK (show_play_time IN (0, 1)),
    warn_on_unknown_modpacks INTEGER NOT NULL DEFAULT TRUE CHECK (warn_on_unknown_modpacks IN (0, 1)),
    skip_non_essential_warnings INTEGER NOT NULL DEFAULT FALSE CHECK (skip_non_essential_warnings IN (0, 1)),
    show_jump_in INTEGER NOT NULL DEFAULT TRUE CHECK (show_jump_in IN (0, 1)),
    always_show_copy_details INTEGER NOT NULL DEFAULT FALSE CHECK (always_show_copy_details IN (0, 1)),
    hide_installed_modpacks INTEGER NOT NULL DEFAULT FALSE CHECK (hide_installed_modpacks IN (0, 1)),
    advanced_filters_collapsed INTEGER NOT NULL DEFAULT TRUE CHECK (advanced_filters_collapsed IN (0, 1)),
    dismissed_photosensitivity_filter_warning INTEGER NOT NULL DEFAULT FALSE CHECK (dismissed_photosensitivity_filter_warning IN (0, 1)),
    friends_active_collapsed INTEGER NOT NULL DEFAULT FALSE CHECK (friends_active_collapsed IN (0, 1)),
    friends_online_collapsed INTEGER NOT NULL DEFAULT FALSE CHECK (friends_online_collapsed IN (0, 1)),
    friends_offline_collapsed INTEGER NOT NULL DEFAULT TRUE CHECK (friends_offline_collapsed IN (0, 1)),
    friends_pending_collapsed INTEGER NOT NULL DEFAULT TRUE CHECK (friends_pending_collapsed IN (0, 1)),

    PRIMARY KEY (id)
);

CREATE TABLE minecraft_users (
    uuid TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT FALSE,
    username TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires INTEGER NOT NULL,

    PRIMARY KEY (uuid)
);

CREATE TABLE minecraft_device_tokens (
    id INTEGER NOT NULL CHECK (id = 0),

    uuid TEXT NOT NULL,
    private_key TEXT NOT NULL,
    x TEXT NOT NULL,
    y TEXT NOT NULL,
    issue_instant INTEGER NOT NULL,
    not_after INTEGER NOT NULL,
    token TEXT NOT NULL,
    display_claims JSONB NOT NULL,

    PRIMARY KEY (id)
);

CREATE TABLE modrinth_users (
    id TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT FALSE,
    session_id TEXT NOT NULL,
    expires INTEGER NOT NULL,

    PRIMARY KEY (id)
);

CREATE TABLE custom_minecraft_skins (
    minecraft_user_uuid TEXT NOT NULL,
    texture_key TEXT NOT NULL,
    variant TEXT NOT NULL CHECK (variant IN ('CLASSIC', 'SLIM', 'UNKNOWN')),
    cape_id TEXT,
    display_order INTEGER NOT NULL DEFAULT 0,

    PRIMARY KEY (minecraft_user_uuid, texture_key, variant, cape_id),
    FOREIGN KEY (texture_key) REFERENCES custom_minecraft_skin_textures(texture_key)
        ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE custom_minecraft_skin_textures (
    texture_key TEXT NOT NULL,
    texture PNG BLOB NOT NULL,

    PRIMARY KEY (texture_key)
);

CREATE TABLE app_metadata (
	key TEXT NOT NULL,
	value TEXT NOT NULL,
	updated_at INTEGER NOT NULL DEFAULT (unixepoch()),

	PRIMARY KEY (key)
);

CREATE TABLE onboarding_checklist (
	id INTEGER NOT NULL CHECK (id = 0),
	has_created_instance INTEGER NOT NULL DEFAULT FALSE CHECK (has_created_instance IN (FALSE, TRUE)),
	has_logged_into_minecraft INTEGER NOT NULL DEFAULT FALSE CHECK (
		has_logged_into_minecraft IN (FALSE, TRUE)
	),
	has_logged_into_modrinth INTEGER NOT NULL DEFAULT FALSE CHECK (
		has_logged_into_modrinth IN (FALSE, TRUE)
	),
	show_checklist INTEGER NOT NULL DEFAULT TRUE CHECK (show_checklist IN (FALSE, TRUE)),
	PRIMARY KEY (id)
);

CREATE TABLE recent_instance_icon_configs (
	background TEXT NOT NULL,
	symbol TEXT NOT NULL,
	used_at INTEGER NOT NULL,

	PRIMARY KEY (background, symbol)
);

CREATE TABLE sync_feature_settings (
	feature TEXT PRIMARY KEY NOT NULL,
	globally_enabled INTEGER NOT NULL CHECK (globally_enabled IN (0, 1)),
	new_instance_default INTEGER NOT NULL CHECK (new_instance_default IN (0, 1))
);

CREATE TABLE instance_sync_preferences (
	instance_id TEXT NOT NULL,
	feature TEXT NOT NULL,
	enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
	PRIMARY KEY (instance_id, feature),
	FOREIGN KEY (feature) REFERENCES sync_feature_settings (feature)
		ON DELETE CASCADE
);

CREATE TABLE instance_sync_checkpoints (
	instance_id TEXT NOT NULL,
	feature TEXT NOT NULL,
	variant TEXT NOT NULL CHECK (variant IN ('default', 'legacy', 'components')),
	expected_sha1 TEXT NOT NULL,
	merge_base BLOB,
	source_revision INTEGER NOT NULL CHECK (source_revision >= 0),
	status TEXT NOT NULL CHECK (status IN ('pending', 'ready')),
	link_mode TEXT CHECK (link_mode IS NULL OR link_mode IN ('copy', 'hard', 'symbolic')),
	PRIMARY KEY (instance_id, feature, variant),
	FOREIGN KEY (instance_id, feature) REFERENCES instance_sync_preferences (instance_id, feature)
		ON DELETE CASCADE
);

CREATE TABLE synced_hotbar_state (
	singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
	schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
	revision INTEGER NOT NULL CHECK (revision >= 0),
	nbt BLOB NOT NULL
);

CREATE TABLE synced_server_state (
	singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
	revision INTEGER NOT NULL CHECK (revision >= 0)
);

CREATE TABLE synced_servers (
	id TEXT PRIMARY KEY NOT NULL,
	position INTEGER NOT NULL UNIQUE CHECK (position >= 0),
	nbt BLOB NOT NULL
);

CREATE TABLE instance_server_projection_entries (
	instance_id TEXT NOT NULL,
	owner TEXT NOT NULL CHECK (owner IN ('synced', 'instance')),
	server_id TEXT NOT NULL,
	nbt BLOB NOT NULL,
	position INTEGER NOT NULL CHECK (position >= 0),
	PRIMARY KEY (instance_id, owner, server_id),
	UNIQUE (instance_id, position)
);

CREATE TABLE instance_server_pack_state (
	instance_id TEXT PRIMARY KEY NOT NULL,
	version_id TEXT
);

CREATE TABLE screenshots (
	id TEXT NOT NULL,
	instance_id TEXT NOT NULL,
	file_name TEXT NOT NULL,
	content_hash TEXT NOT NULL,
	file_size INTEGER NOT NULL,
	modified_at INTEGER NOT NULL,
	created_at INTEGER NOT NULL,
	editor_state TEXT,

	PRIMARY KEY (id),
	UNIQUE (instance_id, file_name)
);

CREATE TABLE screenshot_groups (
	id TEXT NOT NULL,
	name TEXT NOT NULL,
	display_order INTEGER NOT NULL DEFAULT 0,

	PRIMARY KEY (id),
	CHECK (length(trim(name)) > 0)
);

CREATE TABLE screenshot_group_memberships (
	screenshot_id TEXT NOT NULL,
	group_id TEXT NOT NULL,

	PRIMARY KEY (screenshot_id),
	FOREIGN KEY (screenshot_id) REFERENCES screenshots(id) ON DELETE CASCADE,
	FOREIGN KEY (group_id) REFERENCES screenshot_groups(id) ON DELETE CASCADE
);

CREATE TABLE synced_game_option_state (
	singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
	revision INTEGER NOT NULL CHECK (revision >= 0),
	catalog_revision INTEGER NOT NULL CHECK (catalog_revision >= 1)
);

CREATE TABLE synced_game_option_values (
	option_id TEXT PRIMARY KEY NOT NULL,
	kind TEXT NOT NULL CHECK (kind IN ('vanilla', 'external')),
	raw_key TEXT,
	canonical_type TEXT NOT NULL,
	canonical_value_json TEXT,
	value_codec TEXT NOT NULL,
	seeded INTEGER NOT NULL CHECK (seeded IN (0, 1)),
	revision INTEGER NOT NULL CHECK (revision >= 0),
	origin TEXT NOT NULL CHECK (origin IN ('app_editor', 'instance', 'source_seed')),
	source_game_version TEXT,
	source_instance_id TEXT,
	updated_at INTEGER NOT NULL
);

CREATE TABLE synced_game_option_preferences (
	option_id TEXT PRIMARY KEY NOT NULL,
	enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
	source TEXT NOT NULL CHECK (source IN ('catalog_default', 'discovery_default', 'user')),
	revision INTEGER NOT NULL CHECK (revision >= 0)
);

CREATE TABLE instance_game_option_pack_bases (
	instance_id TEXT PRIMARY KEY NOT NULL,
	pack_version_id TEXT,
	source TEXT NOT NULL CHECK (source IN ('client_overrides', 'overrides', 'none')),
	sha1 TEXT,
	encoding TEXT,
	document BLOB
);

CREATE TABLE instance_game_option_update_state (
	instance_id TEXT PRIMARY KEY NOT NULL,
	had_file INTEGER NOT NULL CHECK (had_file IN (0, 1)),
	sha1 TEXT,
	document BLOB
);

CREATE TABLE game_option_locale_origins (
	scope TEXT NOT NULL,
	option_id TEXT NOT NULL,
	source_instance_id TEXT,
	source_game_version TEXT,
	backfilled INTEGER NOT NULL DEFAULT 0,
	observation_json TEXT,
	origin_json TEXT,
	PRIMARY KEY (scope, option_id)
);

CREATE TABLE instance_servers (
	instance_id TEXT NOT NULL,
	id TEXT NOT NULL,
	source TEXT NOT NULL CHECK (source IN ('modpack', 'linked_server_project', 'local_desynced')),
	excluded_synced_server_id TEXT,
	nbt BLOB NOT NULL,
	position INTEGER NOT NULL CHECK (position >= 0),
	PRIMARY KEY (instance_id, id),
	UNIQUE (instance_id, position),
	FOREIGN KEY (excluded_synced_server_id) REFERENCES synced_servers (id)
		ON DELETE SET NULL
);

CREATE TABLE hidden_instances (
	user_id TEXT NOT NULL,
	instance_id TEXT NOT NULL,
	hidden_at INTEGER NOT NULL,

	PRIMARY KEY (user_id, instance_id)
);

CREATE UNIQUE INDEX custom_minecraft_skins_one_per_texture
    ON custom_minecraft_skins (minecraft_user_uuid, texture_key);

CREATE INDEX custom_minecraft_skins_user_display_order
	ON custom_minecraft_skins (minecraft_user_uuid, display_order);

CREATE INDEX recent_instance_icon_configs_used_at
	ON recent_instance_icon_configs(used_at DESC);

CREATE INDEX instance_sync_preferences_feature_enabled ON instance_sync_preferences (
	feature,
	enabled
);

CREATE INDEX screenshots_instance_id ON screenshots(instance_id);

CREATE INDEX screenshots_instance_hash
	ON screenshots(instance_id, content_hash, file_size);

CREATE INDEX screenshot_groups_display_order
	ON screenshot_groups(display_order);

CREATE INDEX screenshot_group_memberships_group_id
	ON screenshot_group_memberships(group_id);

CREATE TRIGGER custom_minecraft_skins_user_uuid_insert_check
    BEFORE INSERT ON custom_minecraft_skins FOR EACH ROW
    BEGIN
        SELECT CASE WHEN NOT EXISTS (
            SELECT 1 FROM minecraft_users WHERE uuid = NEW.minecraft_user_uuid
        ) THEN RAISE(ABORT,
        'Cannot add a custom skin for an unknown Minecraft user UUID') END;
    END;

CREATE TRIGGER custom_minecraft_skins_user_uuid_update_check
    BEFORE UPDATE ON custom_minecraft_skins FOR EACH ROW
    BEGIN
        SELECT CASE WHEN NOT EXISTS (
            SELECT 1 FROM minecraft_users WHERE uuid = NEW.minecraft_user_uuid
        ) THEN RAISE(ABORT,
        'Cannot change a custom skin to refer to an unknown Minecraft user UUID') END;
    END;

CREATE TRIGGER custom_minecraft_skins_user_uuid_update_cascade
    AFTER UPDATE OF uuid ON minecraft_users FOR EACH ROW
    BEGIN
        UPDATE custom_minecraft_skins SET minecraft_user_uuid = NEW.uuid WHERE minecraft_user_uuid = OLD.uuid;
    END;

CREATE TRIGGER custom_minecraft_skin_texture_delete_cleanup
    AFTER DELETE ON custom_minecraft_skins FOR EACH ROW
    BEGIN
        DELETE FROM custom_minecraft_skin_textures WHERE texture_key NOT IN (
            SELECT texture_key FROM custom_minecraft_skins
        );
    END;

CREATE TRIGGER record_game_option_locale_origin
AFTER INSERT ON synced_game_option_values
BEGIN
	INSERT INTO game_option_locale_origins
		(scope, option_id, source_instance_id, source_game_version)
	VALUES ('', NEW.option_id, NEW.source_instance_id, NEW.source_game_version)
	ON CONFLICT(scope, option_id) DO NOTHING;
END;

INSERT INTO settings (id, extra_launch_args, custom_env_vars) VALUES (0, jsonb_array(), jsonb_array());

INSERT INTO onboarding_checklist (id) VALUES (0);

INSERT INTO sync_feature_settings (feature, globally_enabled, new_instance_default)
VALUES
	('command_history', 1, 1),
	('multiplayer_servers', 1, 1),
	('creative_hotbars', 1, 1),
	('screenshots', 1, 1),
	('game_options', 1, 1),
	('resource_packs', 0, 1),
	('data_packs', 0, 1);
