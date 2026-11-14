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
 * A token for one GET of `path` (from the Core's root, with its query) on the Core of a server's
 * node URL, within five minutes, for a download that cannot send the session. Use it with
 * `linkUrl` and the same `path`.
 */
export async function link(client: AbstractModrinthClient, nodeUrlHost: string, path: string) {
	return (await coreOf(client, node(nodeUrlHost).serverId)).call(core.CreateDownloadLink, {
		body: { path },
	})
}

export function linkUrl(nodeUrlHost: string, path: string, token: string): string {
	return `${node(nodeUrlHost).base}${path}${path.includes('?') ? '&' : '?'}link=${token}`
}

/** `path` under the server's own routes, `/servers/{id}`. */
export function serverPath(nodeUrlHost: string, path: string): string {
	return `/servers/${node(nodeUrlHost).serverId}${path}`
}
