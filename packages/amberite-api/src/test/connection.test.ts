import { describe, expect, it, vi } from 'vitest'
import type { PlatformAdapter } from '../adapter'
import { CONNECTION_PROTOCOL, verifyCoreConnection } from '../connection'

function adapter(fetchFn: typeof fetch): PlatformAdapter {
	return {
		fetchFn,
		convexUrl: '',
		getCoreUrl: async () => 'http://localhost:16662',
		getCurrentJwt: async () => null,
		openExternalAuth: vi.fn(),
	}
}

describe('verifyCoreConnection', () => {
	it('connects when Core echoes the nonce and accepts the protocol', async () => {
		const fetchFn = vi.fn(async (_url: string, init?: RequestInit) => {
			const body = JSON.parse(String(init?.body))
			return Response.json({
				nonce: body.nonce,
				ok: true,
				core_id: 'core-1',
				protocol: CONNECTION_PROTOCOL,
				version: '0.1.0',
				reason: null,
			})
		}) as unknown as typeof fetch

		const status = await verifyCoreConnection(adapter(fetchFn))

		expect(status.state).toBe('connected')
		expect(status.coreId).toBe('core-1')
		expect(status.reason).toBeUndefined()
		expect(fetchFn).toHaveBeenCalledWith(
			'http://localhost:16662/connection/handshake',
			expect.objectContaining({ method: 'POST' }),
		)
	})

	it('rejects nonce mismatches', async () => {
		const fetchFn = vi.fn(async () =>
			Response.json({
				nonce: 'wrong',
				ok: true,
				core_id: 'core-1',
				protocol: CONNECTION_PROTOCOL,
				version: '0.1.0',
				reason: null,
			}),
		) as unknown as typeof fetch

		const status = await verifyCoreConnection(adapter(fetchFn))

		expect(status.state).toBe('disconnected')
		expect(status.reason).toBe('nonce-mismatch')
	})

	it('passes wrong-core rejection through', async () => {
		const fetchFn = vi.fn(async (_url: string, init?: RequestInit) => {
			const body = JSON.parse(String(init?.body))
			return Response.json({
				nonce: body.nonce,
				ok: false,
				core_id: 'core-2',
				protocol: CONNECTION_PROTOCOL,
				version: '0.1.0',
				reason: 'wrong-core',
			})
		}) as unknown as typeof fetch

		const status = await verifyCoreConnection(adapter(fetchFn), { knownCoreId: 'core-1' })

		expect(status.state).toBe('disconnected')
		expect(status.coreId).toBe('core-2')
		expect(status.reason).toBe('wrong-core')
	})
})
