import type { AbstractModrinthClient } from '../core/abstract-client'
import type { KyrosContentV1Module } from '../modules/kyros/content/v1'
import type { Adapter } from './adapter'
import { installPackFile, unsupported } from './content'
import { link, linkUrl, serverPath } from './links'

declare module '../modules/kyros/content/v1' {
	interface KyrosContentV1Module {
		/** Core only. A token for one export of the server's content as a `.mrpack`. */
		authorizeMrpackExport(nodeUrlHost: string, worldId: string): Promise<{ token: string }>
		/** Core only. The URL of an authorized export, which the Core writes as it is fetched. */
		getMrpackExportUrl(nodeUrlHost: string, worldId: string, token: string): string
	}
}

const exportMrpack = (nodeUrlHost: string, worldId: string) =>
	serverPath(nodeUrlHost, `/v1/worlds/${encodeURIComponent(worldId)}/content/export-mrpack`)

/** A Core server has one world, with the server's id. */
export function kyrosContent(client: AbstractModrinthClient): Adapter<KyrosContentV1Module> {
	return {
		getEmbeddedAddonIcon: () => unsupported('send the logos inside jars'),
		uploadModpackFile(worldId, file, properties, options) {
			if (!file) return unsupported('keep an earlier upload to install again')
			return installPackFile(
				client,
				worldId,
				file,
				{ backup_id: null },
				properties,
				options?.onProgress,
			)
		},
		authorizeMrpackExport: (nodeUrlHost, worldId) =>
			link(client, nodeUrlHost, exportMrpack(nodeUrlHost, worldId)),
		getMrpackExportUrl: (nodeUrlHost, worldId, token) =>
			linkUrl(nodeUrlHost, exportMrpack(nodeUrlHost, worldId), token),
	}
}
