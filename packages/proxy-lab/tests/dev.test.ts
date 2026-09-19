import assert from 'node:assert/strict'
import { join } from 'node:path'
import test from 'node:test'

import { createDevPlan } from '../src/dev.ts'

test('dev plan keeps each fake user under the dedicated repository data directory', () => {
	const repositoryRoot = join('C:', 'fixture', 'modrinth')
	const owner = createDevPlan(['owner'], repositoryRoot)
	const recipient = createDevPlan(['recipient', '--public-catalog'], repositoryRoot)

	assert.equal(owner.dataDir, join(repositoryRoot, 'modrinthclonedata'))
	assert.equal(owner.serviceDataDir, join(owner.dataDir, 'service'))
	assert.equal(owner.appDataDir, join(owner.dataDir, 'personas', 'owner'))
	assert.equal(owner.env.THESEUS_CONFIG_DIR, owner.appDataDir)
	assert.equal(owner.env.THESEUS_DB_BACKUP_DIR, join(owner.appDataDir, 'backups', 'app-db'))
	assert.equal(owner.env.WEBVIEW2_USER_DATA_FOLDER, join(owner.appDataDir, 'webview'))
	assert.equal(owner.publicCatalog, false)
	assert.equal(recipient.appDataDir, join(recipient.dataDir, 'personas', 'recipient'))
	assert.equal(recipient.publicCatalog, true)
	assert.notEqual(owner.appDataDir, recipient.appDataDir)
})

test('dev plan rejects unknown personas and flags', () => {
	assert.throws(() => createDevPlan(['real-user']), /Unknown proxy-lab persona/)
	assert.throws(() => createDevPlan(['owner', '--write-through']), /Unknown proxy-lab option/)
	assert.throws(() => createDevPlan(['owner', 'recipient']), /accepts one persona/)
})
