import {
	core as coreRoutes,
	type KnownPropertiesFields,
	type Loader,
	type PatchPropertiesFields,
} from '@amberite/contracts/core'
import { instanceContent } from '@amberite/contracts/engine'
import type { AbstractModrinthClient, UploadProgress } from '@modrinth/api-client'

/** What a server is set up with. */
export type ServerSetup =
	| { type: 'loader'; loader: string; loaderVersion: string | null; gameVersion: string }
	| { type: 'modpack'; projectId: string; versionId: string }
	| { type: 'pack-file'; file: Blob }

const LOADERS: Loader[] = ['Vanilla', 'Fabric', 'Quilt', 'Forge', 'NeoForge', 'Paper']

/** The loader a Core installs for the panel's name of one, such as `neoforge`. */
export function serverLoader(name: string): Loader {
	const found = LOADERS.find((each) => each.toLowerCase() === name.replace('_', '').toLowerCase())
	if (!found) throw new Error('A Core installs Vanilla, Fabric, Quilt, Forge, NeoForge, and Paper.')
	return found
}

/** The settings of a creation flow, without the ones it left unset. */
export function knownProperties(
	known: Record<string, string | null | undefined>,
): KnownPropertiesFields {
	return Object.fromEntries(Object.entries(known).filter(([, value]) => typeof value === 'string'))
}

/**
 * Sets server `serverId` up with `setup` once the world settings a creation flow chose are written.
 * A loader is a long op on the Core, which this waits for; a modpack installs as the Core's job.
 */
export async function setUpServer(
	client: AbstractModrinthClient,
	coreId: string,
	serverId: string,
	setup: ServerSetup,
	options: {
		properties?: PatchPropertiesFields
		onUploadProgress?: (progress: UploadProgress) => void
	} = {},
) {
	const core = client.core(coreId)
	const params = { id: serverId }
	if (options.properties)
		await core.call(coreRoutes.ChangeProperties, { params, body: options.properties })
	switch (setup.type) {
		case 'loader':
			return client.cores.op(coreId, (op) =>
				core.call(coreRoutes.InstallLoader, {
					params,
					body: {
						loader: serverLoader(setup.loader),
						loader_version: setup.loaderVersion || null,
						game_version: setup.gameVersion,
						op,
					},
				}),
			)
		case 'modpack':
			await core.call(instanceContent.InstallModpack, {
				params,
				body: { project_id: setup.projectId, version_id: setup.versionId },
			})
			return
		case 'pack-file':
			await core.call(
				coreRoutes.InstallModpackFile,
				{ params, body: setup.file },
				{ onUploadProgress: options.onUploadProgress },
			)
	}
}

/** Installs Modrinth versions on a server, one by one, each without its dependencies. */
export async function addServerVersions(
	client: AbstractModrinthClient,
	coreId: string,
	serverId: string,
	versionIds: string[],
) {
	for (const versionId of versionIds)
		await client.core(coreId).call(instanceContent.AddProjectFromVersion, {
			params: { id: serverId },
			body: { version_id: versionId, reason: 'standalone' },
		})
}
