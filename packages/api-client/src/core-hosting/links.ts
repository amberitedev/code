import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import { getNodeBaseUrl } from '../utils/node-url'
import { coreOf } from './adapter'

/** A Core server's node URL (see `coreNodeUrl`), without a trailing slash, and its server. */
function node(nodeUrlHost: string) {
	const base = getNodeBaseUrl(nodeUrlHost).replace(/\/$/, '')
	const serverId = base.match(/\/servers\/([^/]+)$/)?.[1]
	if (!serverId) throw new ModrinthApiError('This is not a Core server', { statusCode: 400 })
	return { base, serverId }
}

/**
 * A token for one GET of `path` (with its query) under a Core server's node URL, within five
 * minutes, for a download that cannot send the session. Use it with `linkUrl` and the same `path`.
 */
export async function link(client: AbstractModrinthClient, nodeUrlHost: string, path: string) {
	const { serverId } = node(nodeUrlHost)
	return (await coreOf(client, serverId)).call(core.CreateDownloadLink, {
		body: { path: `/servers/${serverId}${path}` },
	})
}

export function linkUrl(nodeUrlHost: string, path: string, token: string): string {
	return `${node(nodeUrlHost).base}${path}${path.includes('?') ? '&' : '?'}link=${token}`
}
