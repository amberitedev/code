import { lstat, readFile, rm, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const GENERATED_MARKER = '# MODRINTH_PROXY_LAB_GENERATED=1'

export function appLibEnvPath(): string {
	return resolve(import.meta.dirname, '../../app-lib/.env')
}

export function generatedAppLibEnv(): string {
	return `${GENERATED_MARKER}
MODRINTH_URL=http://127.0.0.1:8000/
MODRINTH_API_BASE_URL=http://127.0.0.1:8000/
SHARED_INSTANCES_API_BASE_URL=http://127.0.0.1:8000/
MODRINTH_ARCHON_BASE_URL=http://127.0.0.1:8000/
MODRINTH_API_URL=http://127.0.0.1:8000/v2/
MODRINTH_API_URL_V3=http://127.0.0.1:8000/v3/
MODRINTH_SOCKET_URL=ws://127.0.0.1:8000/
MODRINTH_LAUNCHER_META_URL=https://launcher-meta.modrinth.com/
`
}

export async function prepareAppLibEnv(path = appLibEnvPath()): Promise<'created' | 'unchanged'> {
	await refuseLinkedEnvironment(path)
	const expected = generatedAppLibEnv()
	const existing = await readOptional(path)
	if (existing === expected) return 'unchanged'
	if (existing !== null) {
		const kind = existing.includes(GENERATED_MARKER) ? 'modified proxy-lab' : 'existing developer'
		throw new Error(
			`Refusing to overwrite ${kind} environment at ${path}. Move it aside, then run env:prepare again.`,
		)
	}
	await writeFile(path, expected, { encoding: 'utf8', flag: 'wx' })
	return 'created'
}

export async function cleanAppLibEnv(path = appLibEnvPath()): Promise<'removed' | 'absent'> {
	await refuseLinkedEnvironment(path)
	const existing = await readOptional(path)
	if (existing === null) return 'absent'
	if (existing !== generatedAppLibEnv()) {
		throw new Error(`Refusing to remove ${path} because it is not the exact proxy-lab file.`)
	}
	await rm(path)
	return 'removed'
}

async function refuseLinkedEnvironment(path: string): Promise<void> {
	try {
		if ((await lstat(path)).isSymbolicLink()) {
			throw new Error(`Refusing to use linked App environment at ${path}.`)
		}
	} catch (error) {
		if (error instanceof Error && 'code' in error && error.code === 'ENOENT') return
		throw error
	}
}

async function readOptional(path: string): Promise<string | null> {
	try {
		return await readFile(path, 'utf8')
	} catch (error) {
		if (error instanceof Error && 'code' in error && error.code === 'ENOENT') return null
		throw error
	}
}
