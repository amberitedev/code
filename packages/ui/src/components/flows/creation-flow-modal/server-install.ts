import type { AbstractModrinthClient } from '@modrinth/api-client'

import { setUpServer } from '../../../composables/server-setup'
import type { CreationFlowContextValue } from './creation-flow-context'

/** Sets a server up with what a server onboarding flow configured. */
export async function installServerCreationConfig(
	client: AbstractModrinthClient,
	config: CreationFlowContextValue,
	coreId: string,
	serverId: string,
) {
	if (config.projectInstall.value) {
		await config.installServerContent(coreId, serverId)
		return
	}
	const properties = config.buildProperties()
	const file = config.modpackFile.value
	if (config.setupType.value === 'modpack' && file) {
		await config.trackUpload((onUploadProgress) =>
			setUpServer(
				client,
				coreId,
				serverId,
				{ type: 'pack-file', file },
				{ properties, onUploadProgress },
			),
		)
		return
	}
	const selection = config.modpackSelection.value
	await setUpServer(
		client,
		coreId,
		serverId,
		config.setupType.value === 'modpack' && selection
			? { type: 'modpack', projectId: selection.projectId, versionId: selection.versionId }
			: {
					type: 'loader',
					loader: config.selectedLoader.value ?? 'vanilla',
					loaderVersion: config.selectedLoaderVersion.value,
					gameVersion: config.selectedGameVersion.value ?? '',
				},
		{ properties },
	)
}
