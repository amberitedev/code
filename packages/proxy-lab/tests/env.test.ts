import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { cleanAppLibEnv, generatedAppLibEnv, prepareAppLibEnv } from '../src/env.ts'

test('environment preparation never overwrites developer bytes', async () => {
	const directory = await mkdtemp(join(tmpdir(), 'modrinth-proxy-lab-env-'))
	const path = join(directory, '.env')
	try {
		assert.equal(await prepareAppLibEnv(path), 'created')
		assert.equal(await readFile(path, 'utf8'), generatedAppLibEnv())
		assert.equal(await prepareAppLibEnv(path), 'unchanged')
		assert.equal(await cleanAppLibEnv(path), 'removed')

		await writeFile(path, 'DEVELOPER_SETTING=keep-me\n', 'utf8')
		await assert.rejects(
			prepareAppLibEnv(path),
			/Refusing to overwrite existing developer environment/,
		)
		await assert.rejects(cleanAppLibEnv(path), /not the exact proxy-lab file/)
		assert.equal(await readFile(path, 'utf8'), 'DEVELOPER_SETTING=keep-me\n')
	} finally {
		await rm(directory, { recursive: true, force: true })
	}
})
