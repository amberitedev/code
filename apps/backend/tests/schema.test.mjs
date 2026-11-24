import assert from 'node:assert/strict'
import test from 'node:test'

import { startBackend } from './worker.mjs'

// Each index, and the shape of a query in `src/` it serves.
const PLANS = [
	['shared_instances_owner', 'SELECT id FROM shared_instances WHERE owner_id = ?'],
	[
		'shared_instances_home',
		'SELECT id FROM shared_instances i WHERE (i.home = ?1 OR i.replica = ?1)',
	],
	[
		'shared_instances_replica',
		'SELECT id FROM shared_instances i WHERE (i.home = ?1 OR i.replica = ?1)',
	],
	[
		'shared_instance_invites_instance',
		'DELETE FROM shared_instance_invites WHERE instance_id = ? AND expiration <= ?',
	],
	[
		'shared_instance_invites_server',
		'SELECT id FROM shared_instance_invites WHERE core_id = ? AND server_id = ? AND expiration > ?',
	],
	['sessions_refresh_expires', 'DELETE FROM sessions WHERE refresh_expires <= ?'],
	[
		'core_storage_offering',
		'SELECT c.id FROM cores c JOIN core_storage s ON s.core_id = c.id WHERE s.enabled = 1 AND s.cap > s.used AND s.reported > ?1 AND c.cert_fingerprint IS NOT NULL',
	],
]

test('the hot queries use their indexes', async () => {
	const backend = await startBackend('schema')
	try {
		for (const [index, query] of PLANS) {
			const { results } = await backend.db
				.prepare(`EXPLAIN QUERY PLAN ${query.replace(/\?\d*/g, "'x'")}`)
				.all()
			const plan = results.map((row) => row.detail).join('\n')
			assert.ok(plan.includes(`INDEX ${index} (`), `${index}:\n${plan}`)
		}
	} finally {
		await backend.dispose()
	}
})
