import { describe, it, expect, vi } from 'vitest'
import { CoreApiClient } from '../client'
import { CoreOfflineError } from '../errors'
import type { PlatformAdapter } from '../adapter'

const offlineAdapter: PlatformAdapter = {
	fetchFn: vi.fn() as unknown as typeof fetch,
	convexUrl: 'https://test.convex.cloud',
	getCoreUrl: async () => null,
	getCurrentJwt: async () => null,
	openExternalAuth: vi.fn(),
}

const client = new CoreApiClient(offlineAdapter)

describe('CoreApiClient when Core is offline', () => {
	it('listInstances throws CoreOfflineError', async () => {
		await expect(client.listInstances()).rejects.toThrow(CoreOfflineError)
	})

	it('getInstance throws CoreOfflineError', async () => {
		await expect(client.getInstance('00000000-0000-0000-0000-000000000000')).rejects.toThrow(
			CoreOfflineError,
		)
	})

	it('createInstance throws CoreOfflineError', async () => {
		await expect(
			client.createInstance({
				name: 'test',
				game_version: '1.21.1',
				loader: 'vanilla',
				port: 25565,
				memory: { min_mb: 512, max_mb: 1024 },
			}),
		).rejects.toThrow(CoreOfflineError)
	})

	it('deleteInstance throws CoreOfflineError', async () => {
		await expect(client.deleteInstance('any-id')).rejects.toThrow(CoreOfflineError)
	})

	it('start throws CoreOfflineError', async () => {
		await expect(client.start('any-id')).rejects.toThrow(CoreOfflineError)
	})

	it('getStats throws CoreOfflineError', async () => {
		await expect(client.getStats('any-id')).rejects.toThrow(CoreOfflineError)
	})

	it('listMods throws CoreOfflineError', async () => {
		await expect(client.listMods('any-id')).rejects.toThrow(CoreOfflineError)
	})

	it('openConsole throws CoreOfflineError', async () => {
		await expect(client.openConsole('any-id', 'ticket')).rejects.toThrow(CoreOfflineError)
	})

	it('listBackups throws CoreOfflineError', async () => {
		await expect(client.listBackups('any-id')).rejects.toThrow(CoreOfflineError)
	})

	it('does not expose the account JWT when the linked Core identity does not match', async () => {
		const getCurrentJwt = vi.fn(async () => 'account-jwt')
		const fetchFn = vi.fn(async (_url: string, init?: RequestInit) => {
			const body = JSON.parse(String(init?.body))
			return Response.json({
				nonce: body.nonce,
				ok: false,
				core_id: 'unexpected-core',
				protocol: 1,
				version: '0.1.0',
				reason: 'wrong-core',
			})
		}) as unknown as typeof fetch
		const guardedClient = new CoreApiClient({
			fetchFn,
			convexUrl: 'https://test.convex.cloud',
			getCoreUrl: async () => 'https://core.example.com',
			getConnectedCoreId: async () => 'expected-core',
			getCurrentJwt,
			openExternalAuth: vi.fn(),
		})

		await expect(guardedClient.listInstances()).rejects.toThrow(CoreOfflineError)
		expect(getCurrentJwt).not.toHaveBeenCalled()
	})
})
