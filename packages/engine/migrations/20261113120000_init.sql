CREATE TABLE java_versions (
    major_version INTEGER NOT NULL,
    full_version TEXT NOT NULL,
    architecture TEXT NOT NULL,
    path TEXT NOT NULL,

    PRIMARY KEY (major_version)
);

CREATE TABLE cache (
    id TEXT NOT NULL,
    data_type TEXT NOT NULL,
    alias TEXT NULL,

    data JSONB NULL,
    expires INTEGER NOT NULL,

    UNIQUE (data_type, alias),
    PRIMARY KEY (id, data_type)
);

CREATE TABLE instances (
	id TEXT NOT NULL,
	path TEXT NOT NULL,
	applied_content_set_id TEXT NULL,

	install_stage TEXT NOT NULL,
	launcher_feature_version TEXT NOT NULL,
	update_channel TEXT NOT NULL DEFAULT 'release',

	name TEXT NOT NULL,
	icon_path TEXT NULL,

	created INTEGER NOT NULL,
	modified INTEGER NOT NULL,
	last_played INTEGER NULL,

	submitted_time_played INTEGER NOT NULL DEFAULT 0,
	recent_time_played INTEGER NOT NULL DEFAULT 0,

	PRIMARY KEY (id),
	UNIQUE (path)
);

CREATE TABLE instance_links (
	instance_id TEXT NOT NULL,
	link_kind TEXT NOT NULL,

	modrinth_project_id TEXT NULL,
	modrinth_version_id TEXT NULL,

	server_project_id TEXT NULL,

	content_project_id TEXT NULL,
	content_version_id TEXT NULL,

	hosting_server_id TEXT NULL,
	hosting_instance_ids JSONB NULL,
	hosting_active_instance_id TEXT NULL,

	shared_instance_id TEXT NULL,
	imported_name TEXT NULL,
	imported_version_number TEXT NULL,
	imported_filename TEXT NULL,
	shared_instance_role TEXT NULL,
	shared_instance_manager_id TEXT NULL,
	shared_instance_linked_user_id TEXT NULL,
	shared_instance_server_manager_name TEXT NULL,
	shared_instance_server_manager_icon_url TEXT NULL,
	shared_instance_restored_from_remote INTEGER NOT NULL DEFAULT 0,

	PRIMARY KEY (instance_id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE TABLE instance_launch_overrides (
	instance_id TEXT NOT NULL,
	overrides JSONB NOT NULL,

	PRIMARY KEY (instance_id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE TABLE instance_content_sets (
	id TEXT NOT NULL,
	instance_id TEXT NOT NULL,

	name TEXT NOT NULL,
	source_kind TEXT NOT NULL,
	status TEXT NOT NULL,

	game_version TEXT NOT NULL,
	protocol_version INTEGER NULL,
	loader TEXT NOT NULL,
	loader_version TEXT NULL,

	created INTEGER NOT NULL,
	modified INTEGER NOT NULL,

	PRIMARY KEY (id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE TABLE instance_content_set_remote_refs (
	content_set_id TEXT NOT NULL,
	ref_type TEXT NOT NULL,
	ref_id TEXT NOT NULL,

	PRIMARY KEY (content_set_id, ref_type),
	FOREIGN KEY (content_set_id)
		REFERENCES instance_content_sets(id)
		ON DELETE CASCADE
);

CREATE TABLE instance_content_set_sync_state (
	content_set_id TEXT NOT NULL,
	provider TEXT NOT NULL,

	applied_update_id TEXT NULL,
	latest_available_update_id TEXT NULL,
	checked_at INTEGER NULL,
	status TEXT NOT NULL,
	applied_backtrack_version TEXT NULL,

	PRIMARY KEY (content_set_id),
	FOREIGN KEY (content_set_id)
		REFERENCES instance_content_sets(id)
		ON DELETE CASCADE
);

CREATE TABLE instance_files (
	id TEXT NOT NULL,
	instance_id TEXT NOT NULL,

	relative_path TEXT NOT NULL,
	file_name TEXT NOT NULL,
	enabled INTEGER NOT NULL,

	sha1 TEXT NOT NULL,
	size INTEGER NOT NULL,
	missing INTEGER NOT NULL DEFAULT 0,

	added_at INTEGER NOT NULL,
	modified_at INTEGER NOT NULL,

	PRIMARY KEY (id),
	UNIQUE (instance_id, relative_path),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE TABLE instance_content_entries (
	id TEXT NOT NULL,
	instance_id TEXT NOT NULL,
	content_set_id TEXT NOT NULL,
	file_id TEXT NULL,

	project_type TEXT NOT NULL,
	project_id TEXT NULL,
	version_id TEXT NULL,

	source_kind TEXT NOT NULL,
	server_requirement TEXT NOT NULL,
	client_requirement TEXT NOT NULL,
	enabled INTEGER NOT NULL DEFAULT 1,

	added_at INTEGER NOT NULL,
	modified_at INTEGER NOT NULL,

	PRIMARY KEY (id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE,
	FOREIGN KEY (content_set_id)
		REFERENCES instance_content_sets(id)
		ON DELETE CASCADE,
	FOREIGN KEY (file_id) REFERENCES instance_files(id) ON DELETE SET NULL
);

CREATE TABLE instance_content_update_checks (
	content_entry_id TEXT NOT NULL,

	update_channel TEXT NOT NULL,
	update_version_id TEXT NULL,
	checked_at INTEGER NOT NULL,

	PRIMARY KEY (content_entry_id),
	FOREIGN KEY (content_entry_id)
		REFERENCES instance_content_entries(id)
		ON DELETE CASCADE
);

CREATE TABLE processes (
	pid INTEGER NOT NULL,
	start_time INTEGER NOT NULL,
	name TEXT NOT NULL,
	executable TEXT NOT NULL,
	instance_id TEXT NOT NULL,
	post_exit_command TEXT NULL,

	UNIQUE (pid),
	PRIMARY KEY (pid),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE TABLE join_log (
	instance_id TEXT NOT NULL,
	host TEXT NOT NULL,
	port INTEGER NOT NULL,
	join_time INTEGER NOT NULL,

	PRIMARY KEY (instance_id, host, port),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE TABLE attached_world_data (
	instance_id TEXT NOT NULL,
	world_type TEXT CHECK ( world_type in ('singleplayer', 'server') ) NOT NULL,
	world_id TEXT NOT NULL,
	display_status TEXT NOT NULL DEFAULT 'normal',
	project_id TEXT NULL,
	content_kind TEXT NULL,

	PRIMARY KEY (instance_id, world_type, world_id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE TABLE app_metadata (
	key TEXT NOT NULL,
	value TEXT NOT NULL,
	updated_at INTEGER NOT NULL DEFAULT (unixepoch()),

	PRIMARY KEY (key)
);

CREATE TABLE install_jobs (
	id TEXT NOT NULL PRIMARY KEY,
	instance_id TEXT NULL,
	kind TEXT NOT NULL,
	
	status TEXT NOT NULL,
	state JSONB NOT NULL,

	created INTEGER NOT NULL,
	modified INTEGER NOT NULL,
	finished INTEGER NULL,

	dismissed INTEGER NOT NULL DEFAULT 0,
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE SET NULL
);

CREATE TABLE instance_quarantines (
	instance_id TEXT PRIMARY KEY NOT NULL,
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE TABLE instance_groups (
	id TEXT NOT NULL,
	name TEXT NOT NULL,
	display_order INTEGER NOT NULL DEFAULT 0,

	PRIMARY KEY (id)
);

CREATE TABLE instance_group_memberships (
	instance_id TEXT NOT NULL,
	group_id TEXT NOT NULL,

	PRIMARY KEY (instance_id, group_id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE,
	FOREIGN KEY (group_id) REFERENCES instance_groups(id) ON DELETE CASCADE
);

CREATE TABLE instance_icon_configs (
	instance_id TEXT NOT NULL,
	background TEXT NOT NULL,
	symbol TEXT NOT NULL,

	PRIMARY KEY (instance_id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE TABLE instance_content_locks (
	file_id TEXT NOT NULL,

	PRIMARY KEY (file_id),
	FOREIGN KEY (file_id) REFERENCES instance_files(id) ON DELETE CASCADE
);

CREATE TABLE store_blobs (
	sha512 TEXT PRIMARY KEY NOT NULL,
	size INTEGER NOT NULL CHECK (size >= 0),
	status TEXT NOT NULL DEFAULT 'ready' CHECK (status IN ('ready', 'quarantined', 'deleting')),
	modified_as INTEGER NOT NULL,
	created_at INTEGER NOT NULL DEFAULT (unixepoch()),
	last_used_at INTEGER NOT NULL DEFAULT (unixepoch()),
	verified_at INTEGER NOT NULL DEFAULT (unixepoch()),
	sources TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE store_instance_files (
	file_id TEXT PRIMARY KEY NOT NULL REFERENCES instance_files(id) ON DELETE CASCADE,
	blob_sha512 TEXT NOT NULL REFERENCES store_blobs(sha512) ON DELETE RESTRICT,
	materialization_kind TEXT NOT NULL CHECK (materialization_kind IN ('reflink', 'hardlink', 'copy'))
);

CREATE TABLE store_retained_refs (
	owner_kind TEXT NOT NULL,
	owner_id TEXT NOT NULL,
	blob_sha512 TEXT NOT NULL REFERENCES store_blobs(sha512) ON DELETE RESTRICT,
	PRIMARY KEY (owner_kind, owner_id, blob_sha512)
);

CREATE TABLE store_operations (
	id TEXT PRIMARY KEY NOT NULL,
	instance_id TEXT NOT NULL,
	payload TEXT NOT NULL,
	created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE install_job_requests (
	job_id TEXT PRIMARY KEY NOT NULL REFERENCES install_jobs(id) ON DELETE CASCADE,
	request JSONB NOT NULL
);

CREATE TABLE shared_instance_applied_files (
	instance_id TEXT NOT NULL,
	version INTEGER NOT NULL,
	files TEXT NOT NULL,

	PRIMARY KEY (instance_id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE INDEX instances_path ON instances(path);

CREATE INDEX instances_applied_content_set_id
	ON instances(applied_content_set_id);

CREATE INDEX instance_links_link_kind ON instance_links(link_kind);

CREATE INDEX instance_links_modrinth_project_id
	ON instance_links(modrinth_project_id);

CREATE INDEX instance_links_server_project_id
	ON instance_links(server_project_id);

CREATE INDEX instance_links_hosting_server_id
	ON instance_links(hosting_server_id);

CREATE INDEX instance_links_hosting_active_instance_id
	ON instance_links(hosting_active_instance_id);

CREATE INDEX instance_links_shared_instance_id
	ON instance_links(shared_instance_id);

CREATE INDEX instance_content_sets_instance_id
	ON instance_content_sets(instance_id);

CREATE INDEX instance_content_set_remote_refs_ref
	ON instance_content_set_remote_refs(ref_type, ref_id);

CREATE INDEX instance_content_set_sync_state_provider
	ON instance_content_set_sync_state(provider);

CREATE INDEX instance_content_set_sync_state_latest_available_update_id
	ON instance_content_set_sync_state(latest_available_update_id);

CREATE INDEX instance_files_instance_id ON instance_files(instance_id);

CREATE INDEX instance_files_sha1 ON instance_files(sha1);

CREATE INDEX instance_files_missing ON instance_files(missing);

CREATE INDEX instance_content_entries_instance_id
	ON instance_content_entries(instance_id);

CREATE INDEX instance_content_entries_content_set_id
	ON instance_content_entries(content_set_id);

CREATE INDEX instance_content_entries_file_id
	ON instance_content_entries(file_id);

CREATE INDEX instance_content_entries_project_id
	ON instance_content_entries(project_id);

CREATE INDEX instance_content_entries_version_id
	ON instance_content_entries(version_id);

CREATE INDEX instance_content_entries_source_kind
	ON instance_content_entries(source_kind);

CREATE INDEX instance_content_update_checks_update_version_id
	ON instance_content_update_checks(update_version_id);

CREATE INDEX processes_instance_id ON processes(instance_id);

CREATE INDEX join_log_instance_id ON join_log(instance_id);

CREATE INDEX attached_world_data_instance_id
	ON attached_world_data(instance_id);

CREATE INDEX install_jobs_instance_id ON install_jobs(instance_id);

CREATE INDEX install_jobs_status ON install_jobs(status);

CREATE INDEX install_jobs_finished ON install_jobs(finished);

CREATE UNIQUE INDEX instance_content_entries_content_set_file_unique
	ON instance_content_entries(content_set_id, file_id)
	WHERE file_id IS NOT NULL;

CREATE UNIQUE INDEX instance_content_entries_content_set_project_version_unique
	ON instance_content_entries(content_set_id, project_id, version_id)
	WHERE file_id IS NULL
		AND project_id IS NOT NULL
		AND version_id IS NOT NULL;

CREATE INDEX instance_groups_name ON instance_groups(name);

CREATE INDEX instance_groups_display_order ON instance_groups(display_order);

CREATE INDEX instance_group_memberships_group_id
	ON instance_group_memberships(group_id);

CREATE INDEX store_instance_files_blob ON store_instance_files(blob_sha512);

CREATE INDEX store_operations_instance ON store_operations(instance_id);

CREATE INDEX cache_expires ON cache (expires);

INSERT INTO app_metadata (key, value, updated_at)
VALUES ('store_cache_limit_bytes', '5368709120', unixepoch());

INSERT INTO instance_groups (id, name, display_order)
VALUES ('group:favorites', 'Favorites', -1);
