import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, it } from 'vite-plus/test'

import { type App, state } from './context.ts'
import { sql } from './rows.ts'

describe('sql on a program', () => {
	it('returns the rows of any statement, and knows a write by what it changed', async () => {
		const dir = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'fixtures-'))
		const app: App = {
			kind: 'app',
			name: 'alice',
			index: 1,
			dir,
			account: { kind: 'account', name: 'alice', id: 'a' },
		}
		try {
			const db = new DatabaseSync(NodePath.join(dir, 'engine.db'))
			db.exec(
				"CREATE TABLE instances (id TEXT, name TEXT); INSERT INTO instances VALUES ('one', 'Old')",
			)
			db.close()
			state.dirty.clear()

			expect(
				await sql(app, 'WITH named AS (SELECT name FROM instances) SELECT name FROM named'),
			).toEqual([{ name: 'Old' }])
			expect(await sql(app, '-- a comment first\nSELECT id FROM instances')).toEqual([
				{ id: 'one' },
			])
			// Reading leaves the App as it is.
			expect([...state.dirty]).toEqual([])

			expect(
				await sql(app, "INSERT INTO instances VALUES ('two', ?) RETURNING id", ['New']),
			).toEqual([{ id: 'two' }])
			// A changed row means the App is restarted before it is next called.
			expect([...state.dirty]).toEqual(['app:1'])
		} finally {
			NodeFS.rmSync(dir, { force: true, recursive: true })
		}
	})
})
