import { mkdir, readFile, rename, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'

import { createSeedState } from './fixtures.ts'
import type { LabState } from './types.ts'

export type StateStore = {
	dataDir: string
	read(): LabState
	mutate<T>(operation: (state: LabState) => T): Promise<T>
	reset(): Promise<LabState>
}

export async function createStateStore(dataDir: string): Promise<StateStore> {
	const statePath = join(dataDir, 'state.json')
	await mkdir(dataDir, { recursive: true })

	const loadedState = await loadState(statePath)
	let state: LabState = loadedState ?? createSeedState()
	if (!loadedState) {
		await saveState(statePath, state)
	}

	let pendingWrite = Promise.resolve()

	return {
		dataDir,
		read: () => structuredClone(state),
		async mutate<T>(operation: (current: LabState) => T): Promise<T> {
			const result = operation(state)
			pendingWrite = pendingWrite.then(() => saveState(statePath, state))
			await pendingWrite
			return result
		},
		async reset(): Promise<LabState> {
			state = createSeedState()
			pendingWrite = pendingWrite.then(() => saveState(statePath, state))
			await pendingWrite
			return structuredClone(state)
		},
	}
}

async function loadState(path: string): Promise<LabState | null> {
	try {
		const value: unknown = JSON.parse(await readFile(path, 'utf8'))
		if (!isLabState(value)) {
			throw new Error(`Unsupported proxy-lab state in ${path}. Run "pnpm reset".`)
		}
		return value
	} catch (error) {
		if (isNodeError(error) && error.code === 'ENOENT') return null
		throw error
	}
}

async function saveState(path: string, state: LabState): Promise<void> {
	await mkdir(dirname(path), { recursive: true })
	const temporaryPath = `${path}.tmp`
	await writeFile(temporaryPath, `${JSON.stringify(state, null, '\t')}\n`, 'utf8')
	await rename(temporaryPath, path)
}

function isLabState(value: unknown): value is LabState {
	return (
		typeof value === 'object' &&
		value !== null &&
		'schema_version' in value &&
		value.schema_version === 1
	)
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error
}
