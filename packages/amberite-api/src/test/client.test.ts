/**
 * Integration tests for CoreApiClient against a live Copal process.
 *
 * Global setup (src/test/global-setup.ts) ensures Core is running before these
 * tests execute. AMBERITE_DEV defaults to true in debug builds, so no JWT is
 * required.
 *
 * Each test suite is independent where possible. A single instance is created in
 * beforeAll and deleted in afterAll; tests within a suite share that instance.
 */

import { afterAll, beforeAll, describe, expect, it } from 'vitest'

import { CoreApiClient } from '@modrinth/api-client'

const BASE = 'http://localhost:16662'
const client = new CoreApiClient({
	fetchFn: fetch,
	getCoreUrl: async () => BASE,
	getCurrentJwt: async () => null,
})

let instanceId: string
let instancePath: string

beforeAll(async () => {
	const instance = await client.createInstance({
		name: 'integration-test',
		game_version: '1.21.1',
		loader: 'vanilla',
		port: 25600,
		memory: { min_mb: 512, max_mb: 1024 },
	})
	instanceId = instance.id
	instancePath = instance.path
})

afterAll(async () => {
	if (instancePath) {
		await client.deleteInstance(instancePath).catch(() => {})
	}
})

// ── listInstances ──────────────────────────────────────────────────────────────

describe('listInstances', () => {
	it('returns an array that contains the test instance', async () => {
		const instances = await client.listInstances()
		expect(Array.isArray(instances)).toBe(true)
		const found = instances.find((i) => i.id === instanceId)
		expect(found).toBeDefined()
		expect(found!.path).toBe(instancePath)
		expect(found!.name).toBe('integration-test')
	})
})

// ── getInstance ───────────────────────────────────────────────────────────────

describe('getInstance', () => {
	it('returns the full instance with correct fields', async () => {
		const instance = await client.getInstance(instancePath)
		expect(instance.id).toBe(instanceId)
		expect(instance.path).toBe(instancePath)
		expect(instance.name).toBe('integration-test')
		expect(instance.game_version).toBe('1.21.1')
		expect(instance.loader).toBe('vanilla')
		expect(instance.port).toBe(25600)
		expect(instance.status).toBe('offline')
		expect(instance.memory.min_mb).toBe(512)
		expect(instance.memory.max_mb).toBe(1024)
		expect(typeof instance.data_dir).toBe('string')
		expect(instance.data_dir).toContain('integration-test')
	})

	it('throws Core API 404 for a non-existent UUID', async () => {
		await expect(client.getInstance('00000000-0000-0000-0000-000000000000')).rejects.toThrow(
			'Core API 404',
		)
	})
})

// ── renameInstance ────────────────────────────────────────────────────────────

describe('renameInstance', () => {
	it('updates the instance name and returns the updated record', async () => {
		const updated = await client.renameInstance(instancePath, 'integration-test-renamed')
		expect(updated.id).toBe(instanceId)
		expect(updated.path).toBe(instancePath)
		expect(updated.name).toBe('integration-test-renamed')
	})

	it('rename is reflected by a subsequent getInstance call', async () => {
		await client.renameInstance(instancePath, 'integration-test')
		const fetched = await client.getInstance(instancePath)
		expect(fetched.name).toBe('integration-test')
		expect(fetched.path).toBe(instancePath)
	})
})

// ── issueWsTicket ──────────────────────────────────────────────────────────────

describe('issueWsTicket', () => {
	it('returns a non-empty string ticket', async () => {
		const ticket = await client.issueWsTicket()
		expect(typeof ticket).toBe('string')
		expect(ticket.length).toBeGreaterThan(0)
	})
})

// ── getStats ──────────────────────────────────────────────────────────────────

describe('getStats', () => {
	it('returns CoreStats with all-null fields for an offline instance', async () => {
		const stats = await client.getStats(instancePath)
		expect(stats.cpu_percent).toBeNull()
		expect(stats.memory_mb).toBeNull()
		expect(stats.player_count).toBeNull()
		expect(stats.uptime_seconds).toBeNull()
	})

	it('throws Core API 404 for a non-existent UUID', async () => {
		await expect(client.getStats('00000000-0000-0000-0000-000000000000')).rejects.toThrow(
			'Core API 404',
		)
	})
})

// ── getStartup / updateStartup / updateMemory ─────────────────────────────────

describe('getStartup', () => {
	it('returns startup settings with rendered command strings', async () => {
		const startup = await client.getStartup(instancePath)
		expect(typeof startup.memory.min_mb).toBe('number')
		expect(typeof startup.memory.max_mb).toBe('number')
		expect(typeof startup.default_command).toBe('string')
		expect(startup.default_command.length).toBeGreaterThan(0)
		expect(typeof startup.effective_command).toBe('string')
	})

	it('throws Core API 404 for a non-existent UUID', async () => {
		await expect(client.getStartup('00000000-0000-0000-0000-000000000000')).rejects.toThrow(
			'Core API 404',
		)
	})
})

describe('updateMemory', () => {
	it('persists new RAM bounds, reflected in getStartup', async () => {
		await client.updateMemory(instancePath, 1024, 4096)
		const startup = await client.getStartup(instancePath)
		expect(startup.memory.min_mb).toBe(1024)
		expect(startup.memory.max_mb).toBe(4096)
	})
})

describe('updateStartup', () => {
	it('sets and clears jvm_args / server_args', async () => {
		const set = await client.updateStartup(instancePath, {
			jvm_args: '-XX:+UseG1GC',
			server_args: '--nogui',
		})
		expect(set.id).toBe(instanceId)
		expect(set.path).toBe(instancePath)
		const withArgs = await client.getStartup(instancePath)
		expect(withArgs.jvm_args).toBe('-XX:+UseG1GC')
		expect(withArgs.server_args).toBe('--nogui')
		expect(withArgs.effective_command).toContain('-XX:+UseG1GC')

		await client.updateStartup(instancePath, { jvm_args: null, server_args: null })
		const cleared = await client.getStartup(instancePath)
		expect(cleared.jvm_args).toBeNull()
		expect(cleared.server_args).toBeNull()
	})
})

// ── listMods ──────────────────────────────────────────────────────────────────

describe('listMods', () => {
	it('returns an array (empty for a fresh vanilla instance)', async () => {
		const mods = await client.listMods(instancePath)
		expect(Array.isArray(mods)).toBe(true)
	})
})

// ── listLogs / listCrashReports ───────────────────────────────────────────────

describe('listLogs', () => {
	it('returns an array', async () => {
		const logs = await client.listLogs(instancePath)
		expect(Array.isArray(logs)).toBe(true)
	})
})

describe('listCrashReports', () => {
	it('returns an array', async () => {
		const reports = await client.listCrashReports(instancePath)
		expect(Array.isArray(reports)).toBe(true)
	})
})

// ── getProperties / patchProperties ──────────────────────────────────────────

describe('getProperties', () => {
	it('returns a Record with at least a server-port key', async () => {
		const props = await client.getProperties(instancePath)
		expect(typeof props).toBe('object')
		expect(props).not.toBeNull()
		expect('server-port' in props).toBe(true)
	})
})

describe('patchProperties', () => {
	it('patches a key and GET reflects the change', async () => {
		await client.patchProperties(instancePath, { motd: 'Hello Amberite' })
		const props = await client.getProperties(instancePath)
		expect(props['motd']).toBe('Hello Amberite')
	})
})

// ── listBackups ───────────────────────────────────────────────────────────────

describe('listBackups', () => {
	it('returns empty backups and operations for a fresh instance', async () => {
		const result = await client.listBackups(instancePath)
		expect(Array.isArray(result.backups)).toBe(true)
		expect(Array.isArray(result.active_operations)).toBe(true)
	})
})

// ── createBackup → listBackups roundtrip ──────────────────────────────────────

describe('createBackup / listBackups roundtrip', () => {
	it('created backup appears in the list', async () => {
		const backup = await client.createBackup(instancePath, 'test-backup')
		expect(typeof backup.id).toBe('string')
		expect(backup.name).toBe('test-backup')

		const { backups } = await client.listBackups(instancePath)
		const found = backups.find((b) => b.id === backup.id)
		expect(found).toBeDefined()
	})
})

// ── listDirectory ─────────────────────────────────────────────────────────────

describe('listDirectory', () => {
	it('returns a CoreFsListing with correct shape for the instance root', async () => {
		const listing = await client.listDirectory(instancePath, '/')
		expect(Array.isArray(listing.items)).toBe(true)
		expect(typeof listing.total).toBe('number')
		expect(typeof listing.current).toBe('number')
	})
})
