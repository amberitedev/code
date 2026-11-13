import type { AbstractModrinthClient } from '../core/abstract-client'
import type { KyrosContentV1Module } from '../modules/kyros/content/v1'
import type { Adapter } from './adapter'
import { link, linkUrl } from './links'

declare module '../modules/kyros/content/v1' {
	interface KyrosContentV1Module {
		/** Core only. A token for one export of the server's content as a `.mrpack`. */
		authorizeMrpackExport(nodeUrlHost: string, worldId: string): Promise<{ token: string }>
		/** Core only. The URL of an authorized export, which the Core writes as it is fetched. */
		getMrpackExportUrl(nodeUrlHost: string, worldId: string, token: string): string
	}
}

const exportMrpack = (worldId: string) =>
	`/v1/worlds/${encodeURIComponent(worldId)}/content/export-mrpack`

export function kyrosContent(client: AbstractModrinthClient): Adapter<KyrosContentV1Module> {
	return {
		authorizeMrpackExport: (nodeUrlHost, worldId) =>
			link(client, nodeUrlHost, exportMrpack(worldId)),
		getMrpackExportUrl: (nodeUrlHost, worldId, token) =>
			linkUrl(nodeUrlHost, exportMrpack(worldId), token),
	}
}
