import type { Server } from '@amberite/contracts/core'
import type { AbstractModrinthClient } from '@modrinth/api-client'
import { expect, test } from 'vitest'

import { createServerInviteLink } from './invite-link'
import { ShareUnlinkDeclined } from './unlink-before-share'

const uploadedPack = { name: 'Drop Check', version_number: '1.0.0' }

/**
 * A server set up from an uploaded pack, behind a Core that records the routes it is sent. With
 * `installing`, the first read finds the pack still installing and not yet linked, and the install
 * then ends, as the Core announces.
 */
function uploadedPackServer({ installing = false } = {}) {
	const calls: string[] = []
	const server = {
		server_id: 'server',
		name: 'Server',
		status: installing ? 'installing' : 'available',
		shared_instance_id: null,
		uploaded_modpack: installing ? null : uploadedPack,
	} as Server
	const listeners = new Map<string, (payload: unknown) => void>()
	const core = {
		call: async (endpoint: { path: string }) => {
			calls.push(endpoint.path)
			if (endpoint.path.endsWith('/share')) return { shared_instance_id: 'share' }
			if (endpoint.path !== '/servers/{id}') return null
			const read = { ...server }
			if (server.status === 'installing') {
				Object.assign(server, { status: 'available', uploaded_modpack: uploadedPack })
				queueMicrotask(() => listeners.get('server_changed')?.({ server: 'server' }))
			}
			return read
		},
	}
	const client = {
		cores: { serverRoutes: async () => core, coreOf: async () => ({ id: 'core' }) },
		coreEvents: () => ({
			subscribe: () => () => {},
			on: (type: string, listener: (payload: unknown) => void) => {
				listeners.set(type, listener)
				return () => {}
			},
			ready: async () => {},
		}),
		sharedinstances: {
			players_v1: {
				links: async () => [],
				list: async () => ({ users: [] }),
				createLink: async () => ({ id: 'invite' }),
			},
		},
	} as unknown as AbstractModrinthClient
	return { calls, client }
}

test('an uploaded pack is unlinked once the owner confirms, before the server is shared', async () => {
	const { calls, client } = uploadedPackServer()
	const link = await createServerInviteLink(client, 'server', 'https://site', {
		isCurrent: () => true,
		shareUnlink: { ask: async () => true },
	})

	expect(link).toBe('https://site/share/invite')
	expect(calls).toEqual(['/servers/{id}', '/instances/{id}/modpack/unlink', '/servers/{id}/share'])
})

test('a declined unlink shares nothing', async () => {
	const { calls, client } = uploadedPackServer()
	await expect(
		createServerInviteLink(client, 'server', 'https://site', {
			isCurrent: () => true,
			shareUnlink: { ask: async () => false },
		}),
	).rejects.toBeInstanceOf(ShareUnlinkDeclined)

	expect(calls).toEqual(['/servers/{id}'])
})

test('after a pack upload, the invite waits for the install to end before it unlinks', async () => {
	const { calls, client } = uploadedPackServer({ installing: true })
	const link = await createServerInviteLink(client, 'server', 'https://site', {
		isCurrent: () => true,
		shareUnlink: { ask: async () => true },
		afterInstall: true,
	})

	expect(link).toBe('https://site/share/invite')
	expect(calls).toEqual([
		'/servers/{id}',
		'/servers/{id}',
		'/instances/{id}/modpack/unlink',
		'/servers/{id}/share',
	])
})
