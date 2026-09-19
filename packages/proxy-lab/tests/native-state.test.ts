import assert from 'node:assert/strict'
import { mkdtemp, mkdir, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import test from 'node:test'

import { PERSONAS } from '../src/fixtures.ts'
import { prepareNativePersona } from '../src/native-state.ts'

test('native App state is migrated and seeded with the recipient and shared fixture', async () => {
	const directory = await mkdtemp(join(tmpdir(), 'modrinth-proxy-lab-native-'))
	const appDataDir = join(directory, 'owner')
	await mkdir(appDataDir)
	try {
		await prepareNativePersona(appDataDir, 'recipient', resolve(import.meta.dirname, '../../..'))
		const database = new DatabaseSync(join(appDataDir, 'app.db'), { readOnly: true })
		try {
			const credentials = database
				.prepare('SELECT id, active, session_id FROM modrinth_users')
				.all()
				.map((row) => ({ ...row }))
			assert.deepEqual(credentials, [
				{
					id: PERSONAS.recipient.user.id,
					active: 1,
					session_id: PERSONAS.recipient.token,
				},
			])
			const settings = {
				...database
					.prepare(
						'SELECT telemetry, show_files_tab_in_instances, show_worlds_tab_in_instances, show_screenshots_tab_in_instances FROM settings WHERE id = 0',
					)
					.get(),
			}
			assert.deepEqual(settings, {
				telemetry: 0,
				show_files_tab_in_instances: 1,
				show_worlds_tab_in_instances: 1,
				show_screenshots_tab_in_instances: 1,
			})
			assert.deepEqual(
				{
					...database
						.prepare('SELECT name, path, applied_content_set_id FROM instances WHERE id = ?')
						.get('proxy-lab-shared-member'),
				},
				{
					name: 'Proxy Lab Shared Pack',
					path: 'proxy-lab-shared-member',
					applied_content_set_id: 'proxy-lab-shared-member-content',
				},
			)
		} finally {
			database.close()
		}
	} finally {
		await rm(directory, { recursive: true, force: true })
	}
})
