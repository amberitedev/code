import type { AbstractModrinthClient } from '../core/abstract-client'
import type { KyrosContentV1Module } from '../modules/kyros/content/v1'
import type { Adapter } from './adapter'
import { installPackFile, unsupported } from './content'
import { authorize, coreUrl, serverPath } from './links'

declare module '../modules/kyros/content/v1' {
	interface KyrosContentV1Module {
		/** Core only. Signs in again if needed; the export goes through the App's Core proxy. */
		authorizeMrpackExport(nodeUrlHost: string, worldId: string): Promise<{ token: string }>
		/** Core only. The URL of the export, which the Core writes as it is fetched. */
		getMrpackExportUrl(nodeUrlHost: string, worldId: string, token: string): string
	}
}

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
		authorizeMrpackExport: (nodeUrlHost) => authorize(client, nodeUrlHost),
		getMrpackExportUrl: (nodeUrlHost) =>
			coreUrl(nodeUrlHost, serverPath(nodeUrlHost, '/export.mrpack')),
	}
}
