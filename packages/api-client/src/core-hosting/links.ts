import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import { getNodeBaseUrl } from '../utils/node-url'
import { coreOf } from './adapter'

/** A Core server's node URL (see `coreNodeUrl`): its Core's base, without a trailing slash, and the server. */
export function node(nodeUrlHost: string) {
	const match = getNodeBaseUrl(nodeUrlHost)
		.replace(/\/$/, '')
		.match(/^(.*)\/servers\/([^/]+)$/)
	if (!match) throw new ModrinthApiError('This is not a Core server', { statusCode: 400 })
	return { base: match[1], serverId: match[2] }
}

/**
 * The URL of `path` (from the Core's root, with its query) on the Core of a server's node URL.
 * It goes through the App's Core proxy, which adds this device's session, so a native save needs
 * no token.
 */
export function coreUrl(nodeUrlHost: string, path: string): string {
	return `${node(nodeUrlHost).base}${path}`
}

/**
 * Nothing to authorize: a download goes through the App's Core proxy, which adds this device's
 * session. One call first signs in again when that session ended, so the download has one.
 */
export async function authorize(client: AbstractModrinthClient, nodeUrlHost: string) {
	const { serverId } = node(nodeUrlHost)
	await (await coreOf(client, serverId)).call(core.GetServer, { params: { id: serverId } })
	return { token: '' }
}

/** `path` under the server's own routes, `/servers/{id}`. */
export function serverPath(nodeUrlHost: string, path: string): string {
	return `/servers/${node(nodeUrlHost).serverId}${path}`
}
