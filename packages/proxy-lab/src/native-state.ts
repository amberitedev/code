import { createHash } from 'node:crypto'
import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { PERSONAS } from './fixtures.ts'
import type { Persona } from './types.ts'

type AppliedMigration = {
	version: number
	checksum: Uint8Array
}

export async function prepareNativePersona(
	appDataDir: string,
	personaName: Persona['id'],
	repositoryRoot: string,
): Promise<void> {
	const database = new DatabaseSync(join(appDataDir, 'app.db'))
	try {
		database.exec(
			'PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 30000;',
		)
		await applyMigrations(database, join(repositoryRoot, 'packages', 'app-lib', 'migrations'))
		seedPersona(database, personaName)
		if (personaName === 'recipient') await seedSharedMemberInstance(database, appDataDir)
	} finally {
		database.close()
	}
}

async function seedSharedMemberInstance(database: DatabaseSync, appDataDir: string): Promise<void> {
	const instanceId = 'proxy-lab-shared-member'
	const instancePath = 'proxy-lab-shared-member'
	const contentSetId = 'proxy-lab-shared-member-content'
	const now = Math.floor(Date.now() / 1_000)
	const modsDir = join(appDataDir, 'profiles', instancePath, 'mods')
	await mkdir(modsDir, { recursive: true })
	const fixtureBytes = Buffer.from('UEsFBgAAAAAAAAAAAAAAAAAAAAAAAA==', 'base64')
	await writeFile(join(modsDir, 'proxy-lab-empty.jar'), fixtureBytes)

	database.exec('BEGIN IMMEDIATE')
	try {
		database
			.prepare(
				`
				INSERT INTO instances (
					id, path, applied_content_set_id, install_stage, launcher_feature_version,
					update_channel, name, created, modified
				)
				VALUES (?, ?, ?, 'installed', 'migrated_launch_hooks', 'release', ?, ?, ?)
				ON CONFLICT (id) DO UPDATE SET
					applied_content_set_id = excluded.applied_content_set_id,
					name = excluded.name,
					modified = excluded.modified
			`,
			)
			.run(instanceId, instancePath, contentSetId, 'Proxy Lab Shared Pack', now, now)
		database
			.prepare(
				`
				INSERT INTO instance_content_sets (
					id, instance_id, name, source_kind, status, game_version,
					protocol_version, loader, loader_version, created, modified
				)
				VALUES (?, ?, 'Shared content', 'shared_instance', 'available', '1.21.1', NULL, 'fabric', '0.16.10', ?, ?)
				ON CONFLICT (id) DO UPDATE SET modified = excluded.modified
			`,
			)
			.run(contentSetId, instanceId, now, now)
		database
			.prepare(
				`
				INSERT INTO instance_links (
					instance_id, link_kind, shared_instance_id, shared_instance_role,
					shared_instance_manager_id, shared_instance_linked_user_id,
					shared_instance_server_manager_name
				)
				VALUES (?, 'shared_instance', 'proxy-lab-shared-instance', 'member',
					'proxy-lab-server', 'proxy-lab-recipient', 'Proxy Lab Lighthouse')
				ON CONFLICT (instance_id) DO UPDATE SET
					link_kind = excluded.link_kind,
					shared_instance_id = excluded.shared_instance_id,
					shared_instance_role = excluded.shared_instance_role,
					shared_instance_manager_id = excluded.shared_instance_manager_id,
					shared_instance_linked_user_id = excluded.shared_instance_linked_user_id,
					shared_instance_server_manager_name = excluded.shared_instance_server_manager_name
			`,
			)
			.run(instanceId)
		database
			.prepare(
				`
				INSERT INTO instance_content_set_sync_state (
					content_set_id, provider, applied_update_id, latest_available_update_id, checked_at, status
				)
				VALUES (?, 'shared_instance', '1', '1', ?, 'up_to_date')
				ON CONFLICT (content_set_id) DO UPDATE SET
					applied_update_id = excluded.applied_update_id,
					latest_available_update_id = excluded.latest_available_update_id,
					checked_at = excluded.checked_at,
					status = excluded.status
			`,
			)
			.run(contentSetId, now)
		database
			.prepare(
				`
				INSERT INTO instance_sync_preferences (instance_id, feature, enabled)
				SELECT ?, feature, new_instance_default FROM sync_feature_settings
				WHERE TRUE
				ON CONFLICT (instance_id, feature) DO NOTHING
			`,
			)
			.run(instanceId)
		database.exec('COMMIT')
	} catch (error) {
		database.exec('ROLLBACK')
		throw error
	}
}

async function applyMigrations(database: DatabaseSync, directory: string): Promise<void> {
	database.exec(`
		CREATE TABLE IF NOT EXISTS _sqlx_migrations (
			version BIGINT PRIMARY KEY,
			description TEXT NOT NULL,
			installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
			success BOOLEAN NOT NULL,
			checksum BLOB NOT NULL,
			execution_time BIGINT NOT NULL
		)
	`)
	const applied = new Map(
		database
			.prepare('SELECT version, checksum FROM _sqlx_migrations WHERE success = TRUE')
			.all()
			.map((row) => {
				const migration = row as AppliedMigration
				return [migration.version, Buffer.from(migration.checksum).toString('hex')]
			}),
	)
	const files = (await readdir(directory)).filter((file) => file.endsWith('.sql')).sort()

	for (const file of files) {
		const match = /^(\d+)_([^/]+)\.sql$/.exec(file)
		if (!match) throw new Error(`Unsupported App migration filename ${file}`)
		const version = Number(match[1])
		const description = match[2]!.replaceAll('_', ' ')
		const sql = await readFile(join(directory, file))
		const checksum = createHash('sha384').update(sql).digest()
		const existingChecksum = applied.get(version)
		if (existingChecksum) {
			if (existingChecksum !== checksum.toString('hex')) {
				throw new Error(`App migration ${version} changed after it was applied.`)
			}
			continue
		}

		const started = process.hrtime.bigint()
		database.exec('BEGIN IMMEDIATE')
		try {
			database.exec(sql.toString('utf8'))
			const executionTime = process.hrtime.bigint() - started
			database
				.prepare(
					'INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, ?, TRUE, ?, ?)',
				)
				.run(version, description, checksum, executionTime)
			database.exec('COMMIT')
		} catch (error) {
			database.exec('ROLLBACK')
			throw error
		}
	}
}

function seedPersona(database: DatabaseSync, personaName: Persona['id']): void {
	const persona = PERSONAS[personaName]
	const expires = Math.floor(Date.now() / 1_000) + 60 * 60 * 24 * 365

	database.exec('BEGIN IMMEDIATE')
	try {
		database.prepare('UPDATE modrinth_users SET active = FALSE').run()
		database
			.prepare(
				`
				INSERT INTO modrinth_users (id, active, session_id, expires)
				VALUES (?, TRUE, ?, ?)
				ON CONFLICT (id) DO UPDATE SET
					active = TRUE,
					session_id = excluded.session_id,
					expires = excluded.expires
			`,
			)
			.run(persona.user.id, persona.token, expires)
		database
			.prepare(
				`
				UPDATE settings
				SET telemetry = FALSE,
					show_files_tab_in_instances = TRUE,
					show_worlds_tab_in_instances = TRUE,
					show_screenshots_tab_in_instances = TRUE
				WHERE id = 0
			`,
			)
			.run()
		database
			.prepare(
				`
				UPDATE onboarding_checklist
				SET has_logged_into_modrinth = TRUE
				WHERE id = 0
			`,
			)
			.run()
		database.exec('COMMIT')
	} catch (error) {
		database.exec('ROLLBACK')
		throw error
	}
}
