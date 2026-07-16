import assert from 'node:assert/strict'
import test from 'node:test'

import { makeShortId, validateDevUsername } from './dev-shared.mjs'

test('development usernames are validated directly', () => {
	assert.equal(validateDevUsername('owner'), 'owner')
	assert.throws(() => validateDevUsername('not a username'), /usernames/)
})

test('generated app IDs avoid active IDs', () => {
	const existing = new Set(['000000'])
	const id = makeShortId(existing)
	assert.match(id, /^[a-f0-9]{6}$/)
	assert.equal(existing.has(id), false)
})
