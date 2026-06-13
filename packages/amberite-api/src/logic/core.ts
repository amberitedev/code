import type { CoreApiClient } from '../client'
import type {
	CoreCreateInstanceBody,
	CoreInstance,
	CoreInstanceSummary,
	CoreMemory,
	CoreModLoader,
	CoreModpackManifest,
} from '../types'

export interface CoreProvisionProfile {
	name: string
	gameVersion: string
	modloader: string
	loaderVersion?: string | null
}

export interface InstallCoreModpackOptions {
	projectId: string
	versionId: string
	profile: CoreProvisionProfile
	existingInstances?: Array<Pick<CoreInstanceSummary, 'port'>>
	memory?: CoreMemory
	port?: number
}

export interface InstallCoreModpackResult {
	instance: CoreInstance
	manifest: CoreModpackManifest
}

export interface CreateCoreInstanceFromProfileOptions {
	profile: CoreProvisionProfile
	existingInstances?: Array<Pick<CoreInstanceSummary, 'port'>>
	memory?: CoreMemory
	port?: number
}

export function normalizeCoreLoader(loader: string | null | undefined): CoreModLoader {
	if (loader === 'purpur') return 'paper'
	if (['vanilla', 'paper', 'fabric', 'forge', 'neoforge', 'quilt'].includes(loader ?? '')) {
		return loader as CoreModLoader
	}
	return 'vanilla'
}

export function getNextCoreServerPort(ports: number[], startPort = 25565): number {
	const usedPorts = new Set(ports)
	let port = startPort
	while (usedPorts.has(port)) port += 1
	return port
}

export function createCoreInstanceBodyFromProfile(
	profile: CoreProvisionProfile,
	port: number,
	memory: CoreMemory = { min_mb: 1024, max_mb: 4096 },
): CoreCreateInstanceBody {
	return {
		name: profile.name,
		game_version: profile.gameVersion,
		loader: normalizeCoreLoader(profile.modloader),
		loader_version: profile.loaderVersion ?? undefined,
		port,
		memory,
	}
}

export async function createCoreInstanceFromProfile(
	client: CoreApiClient,
	options: CreateCoreInstanceFromProfileOptions,
): Promise<CoreInstance> {
	const existingInstances = options.existingInstances ?? (await client.listInstances())
	const port =
		options.port ?? getNextCoreServerPort(existingInstances.map((instance) => instance.port))
	return await client.createInstance(
		createCoreInstanceBodyFromProfile(options.profile, port, options.memory),
	)
}

export async function installCoreModpack(
	client: CoreApiClient,
	options: InstallCoreModpackOptions,
): Promise<InstallCoreModpackResult> {
	const instance = await createCoreInstanceFromProfile(client, options)
	const manifest = await client.installModpackVersion(
		instance.id,
		options.projectId,
		options.versionId,
	)
	return { instance, manifest }
}
