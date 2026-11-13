import {
	type DesyncServerMode,
	type DisplayStatus,
	type InstancePayload,
	launch,
	type ProtocolVersion,
	type ServerPackStatus,
	type ServerSource,
	type ServerStatus,
	type SingleplayerGameMode,
	worlds as worldRoutes,
	type WorldType,
} from '@modrinth/api-client'
import type { GameVersion } from '@modrinth/ui'
import { autoToHTML } from '@sfirew/minecraft-motd-parser'
import dayjs from 'dayjs'

import { get_full_path } from '@/helpers/instance'
import { openPath } from '@/helpers/utils'
import { localFileUrl } from '@/platform/local'
import { apiClient } from '@/platform/modrinth-client'

export type {
	DesyncServerMode,
	DisplayStatus,
	ProtocolVersion,
	ServerPackStatus,
	ServerSource,
	ServerStatus,
	SingleplayerGameMode,
	WorldType,
}

type BaseWorld = {
	name: string
	last_played?: string
	icon?: string
	display_status: DisplayStatus
	type: WorldType
}

export type SingleplayerWorld = BaseWorld & {
	type: 'singleplayer'
	path: string
	game_mode: SingleplayerGameMode
	hardcore: boolean
	locked: boolean
}

export type ServerWorld = BaseWorld & {
	type: 'server'
	index: number
	server_id?: string
	source?: ServerSource
	address: string
	pack_status: ServerPackStatus
	project_id?: string
	content_kind?: string
}

export type World = SingleplayerWorld | ServerWorld

export type WorldWithInstance = {
	instance_id: string
} & World

export async function desync_server(
	instanceId: string,
	serverId: string,
	mode: DesyncServerMode,
): Promise<void> {
	return await apiClient.local.call(worldRoutes.DesyncInstanceServer, {
		params: { id: instanceId },
		body: { server_id: serverId, mode },
	})
}

export type ServerData = {
	refreshing: boolean
	lastSuccessfulRefresh?: number
	status?: ServerStatus
	// https://minecraft.wiki/w/Text_component_format
	rawMotd?: ServerStatus['description']
	renderedMotd?: string
}

/** A singleplayer world's icon arrives as a path; a server's is a `data:` URL. */
function withIconUrl<W extends World>(world: W): W {
	return world.type === 'singleplayer' && world.icon
		? { ...world, icon: localFileUrl(world.icon) }
		: world
}

export async function get_recent_worlds(
	limit: number,
	displayStatuses?: DisplayStatus[],
): Promise<WorldWithInstance[]> {
	const worlds = await apiClient.local.call(worldRoutes.ListRecentWorlds, {
		query: { limit, display_statuses: displayStatuses ? JSON.stringify(displayStatuses) : null },
	})
	return worlds.map(withIconUrl)
}

export async function get_instance_worlds(instanceId: string): Promise<World[]> {
	const worlds = await apiClient.local.call(worldRoutes.ListInstanceWorlds, {
		params: { id: instanceId },
	})
	return worlds.map(withIconUrl)
}

export async function get_singleplayer_world(
	instance: string,
	world: string,
): Promise<SingleplayerWorld> {
	const found = await apiClient.local.call(worldRoutes.GetSingleplayerWorld, {
		params: { id: instance, world },
	})
	if (found.type !== 'singleplayer') throw new Error(`${world} is not a singleplayer world`)
	return withIconUrl(found)
}

export async function set_world_display_status(
	instance: string,
	worldType: WorldType,
	worldId: string,
	displayStatus: DisplayStatus,
): Promise<void> {
	return await apiClient.local.call(worldRoutes.SetWorldDisplayStatus, {
		params: { id: instance },
		body: { world_type: worldType, world_id: worldId, display_status: displayStatus },
	})
}

export async function rename_world(
	instance: string,
	world: string,
	newName: string,
): Promise<void> {
	return await apiClient.local.call(worldRoutes.RenameWorld, {
		params: { id: instance, world },
		body: { name: newName },
	})
}

export async function reset_world_icon(instance: string, world: string): Promise<void> {
	return await apiClient.local.call(worldRoutes.ResetWorldIcon, { params: { id: instance, world } })
}

export async function backup_world(instance: string, world: string): Promise<number> {
	return await apiClient.local.call(worldRoutes.BackupWorld, { params: { id: instance, world } })
}

export async function delete_world(instance: string, world: string): Promise<void> {
	return await apiClient.local.call(worldRoutes.DeleteWorld, { params: { id: instance, world } })
}

export async function add_server_to_instance(
	instanceId: string,
	name: string,
	address: string,
	packStatus: ServerPackStatus,
	projectId?: string,
	contentKind?: string,
): Promise<number> {
	return await apiClient.local.call(worldRoutes.AddInstanceServer, {
		params: { id: instanceId },
		body: {
			name,
			address,
			pack_status: packStatus,
			project_id: projectId ?? null,
			content_kind: contentKind ?? null,
		},
	})
}

export async function edit_server_in_instance(
	instanceId: string,
	index: number,
	name: string,
	address: string,
	packStatus: ServerPackStatus,
): Promise<void> {
	return await apiClient.local.call(worldRoutes.EditInstanceServer, {
		params: { id: instanceId, index },
		body: { name, address, pack_status: packStatus },
	})
}

export async function remove_server_from_instance(
	instanceId: string,
	index: number,
): Promise<void> {
	return await apiClient.local.call(worldRoutes.RemoveInstanceServer, {
		params: { id: instanceId, index },
	})
}

export async function get_instance_protocol_version(
	instanceId: string,
): Promise<ProtocolVersion | null> {
	return await apiClient.local.call(worldRoutes.GetInstanceProtocolVersion, {
		params: { id: instanceId },
	})
}

export async function get_server_status(
	address: string,
	protocolVersion: ProtocolVersion | null = null,
): Promise<ServerStatus> {
	return await apiClient.local.call(worldRoutes.GetServerStatus, {
		query: {
			address,
			protocol_version: protocolVersion?.version ?? null,
			legacy: protocolVersion?.legacy ?? false,
		},
	})
}

export async function start_join_singleplayer_world(
	instanceId: string,
	world: string,
): Promise<unknown> {
	return await apiClient.local.call(launch.joinSingleplayerWorld, {
		body: { instance_id: instanceId, world },
	})
}

export async function start_join_server(instanceId: string, address: string): Promise<unknown> {
	return await apiClient.local.call(launch.joinServer, {
		body: { instance_id: instanceId, address },
	})
}

export async function showWorldInFolder(instanceId: string, worldPath: string) {
	const fullPath = await get_full_path(instanceId)
	return await openPath(fullPath + '/saves/' + worldPath)
}

export function getWorldIdentifier(world: World) {
	return world.type === 'singleplayer' ? world.path : world.address
}

export function sortWorlds(worlds: World[]) {
	worlds.sort((a, b) => {
		if (!a.last_played) {
			return 1
		}
		if (!b.last_played) {
			return -1
		}
		return dayjs(b.last_played).diff(dayjs(a.last_played))
	})
}

export function isSingleplayerWorld(world: World): world is SingleplayerWorld {
	return world.type === 'singleplayer'
}

export function isServerWorld(world: World): world is ServerWorld {
	return world.type === 'server'
}

const MINECRAFT_FORMATTING_CODE = /\u00A7[0-9A-FK-ORX]/gi

export function stripMinecraftFormatting(text: string): string {
	return text.replace(MINECRAFT_FORMATTING_CODE, '').replace(/\u00A7/g, '')
}

export function getWorldDisplayName(world: World): string {
	const name = stripMinecraftFormatting(world.name).trim()
	if (name) {
		return name
	}
	if (world.type === 'server') {
		return world.address
	}
	return world.path
}

export function worldNameMatchesQuery(name: string, query: string): boolean {
	const normalizedQuery = query.trim().toLocaleLowerCase()
	if (!normalizedQuery) return true
	return (
		name.toLocaleLowerCase().includes(normalizedQuery) ||
		stripMinecraftFormatting(name).toLocaleLowerCase().includes(normalizedQuery)
	)
}

const DEFAULT_MINECRAFT_SERVER_PORT = 25565

function parseServerPort(port: string): number | null {
	const parsed = Number.parseInt(port, 10)
	return Number.isInteger(parsed) && parsed > 0 && parsed <= 65535 ? parsed : null
}

function parseServerHost(address: string): string {
	const trimmedAddress = address.trim()
	if (!trimmedAddress) return ''

	if (trimmedAddress.startsWith('[')) {
		const closingBracket = trimmedAddress.indexOf(']')
		if (closingBracket > 0) {
			return trimmedAddress.slice(1, closingBracket).trim().toLowerCase()
		}
	}

	const firstColon = trimmedAddress.indexOf(':')
	const lastColon = trimmedAddress.lastIndexOf(':')

	if (firstColon !== -1 && firstColon === lastColon) {
		return trimmedAddress.slice(0, firstColon).trim().toLowerCase()
	}

	return trimmedAddress.toLowerCase()
}

/**
 * Normalization converts addresses to a canonical form (lowercase-host:port, default port 25565)
 */
export function normalizeServerAddress(address: string): string {
	const trimmedAddress = address.trim()
	const host = parseServerHost(trimmedAddress)
	if (!host) return ''
	let port = DEFAULT_MINECRAFT_SERVER_PORT

	// ipv6 address
	if (trimmedAddress.startsWith('[')) {
		const closingBracket = trimmedAddress.indexOf(']')
		if (closingBracket > 0) {
			const suffix = trimmedAddress.slice(closingBracket + 1)
			if (suffix.startsWith(':')) {
				const parsedPort = parseServerPort(suffix.slice(1))
				if (parsedPort != null) {
					port = parsedPort
				}
			}
		}

		// ipv4 address or hostname
	} else {
		const firstColon = trimmedAddress.indexOf(':')
		const lastColon = trimmedAddress.lastIndexOf(':')
		if (firstColon !== -1 && firstColon === lastColon) {
			const parsedPort = parseServerPort(trimmedAddress.slice(firstColon + 1))
			if (parsedPort != null) {
				port = parsedPort
			}
		}
	}

	return `${host}:${port}`
}
export function resolveManagedServerWorld(
	worlds: World[],
	managedName: string | null | undefined,
	managedAddress: string | null | undefined,
): ServerWorld | null {
	if (!managedName || !managedAddress) return null

	const normalizedManagedAddress = normalizeServerAddress(managedAddress)
	if (!normalizedManagedAddress) return null

	const servers = worlds
		.filter(isServerWorld)
		.slice()
		.sort((a, b) => a.index - b.index)

	const exactMatch = servers.find(
		(server) =>
			server.name === managedName &&
			normalizeServerAddress(server.address) === normalizedManagedAddress,
	)
	if (exactMatch) return exactMatch

	return (
		servers.find((server) => normalizeServerAddress(server.address) === normalizedManagedAddress) ??
		null
	)
}

export function getServerAddress(javaServer?: { address?: string | null } | null) {
	if (!javaServer) return null
	return javaServer.address ?? null
}

export async function ensureManagedServerWorldExists(
	instanceId: string,
	serverName: string | null,
	serverAddress: string | null,
) {
	if (!instanceId || !serverName || !serverAddress) return
	try {
		await apiClient.local.call(worldRoutes.EnsureManagedInstanceServer, {
			params: { id: instanceId },
			body: { name: serverName, address: serverAddress },
		})
	} catch (err) {
		console.error('Failed to ensure managed server world exists:', err)
	}
}

export async function getServerLatency(
	address: string,
	protocolVersion: ProtocolVersion | null = null,
): Promise<number | undefined> {
	const pings: number[] = []
	for (let i = 0; i < 3; i++) {
		try {
			const status = await get_server_status(address, protocolVersion)
			if (status.ping != null) {
				pings.push(status.ping)
			}
		} catch {
			// Ignore individual ping failures
		}
	}
	if (pings.length === 0) return undefined
	return Math.round(pings.reduce((sum, p) => sum + p, 0) / pings.length)
}

export async function refreshServerData(
	serverData: ServerData,
	protocolVersion: ProtocolVersion | null,
	address: string,
): Promise<void> {
	const refreshTime = Date.now()
	serverData.refreshing = true
	try {
		const status = await get_server_status(address, protocolVersion)
		if (serverData.lastSuccessfulRefresh && serverData.lastSuccessfulRefresh > refreshTime) {
			// Don't update if there was a more recent successful refresh
			return
		}
		serverData.lastSuccessfulRefresh = Date.now()
		serverData.status = status
		if (status.description) {
			serverData.rawMotd = status.description
			serverData.renderedMotd = autoToHTML(status.description)
		} else {
			delete serverData.rawMotd
			delete serverData.renderedMotd
		}
	} catch (err) {
		console.error(`Refreshing addr ${address}`, protocolVersion, err)
		if (!protocolVersion?.legacy) {
			await refreshServerData(serverData, { version: 74, legacy: true }, address)
			return
		}
		if (!serverData.lastSuccessfulRefresh || serverData.lastSuccessfulRefresh <= refreshTime) {
			delete serverData.status
			delete serverData.rawMotd
			delete serverData.renderedMotd
		}
	} finally {
		serverData.refreshing = false
	}
}

export async function refreshServers(
	worlds: World[],
	serverData: Record<string, ServerData>,
	protocolVersion: ProtocolVersion | null,
	ping = true,
): Promise<void> {
	const servers = worlds.filter(isServerWorld)
	servers.forEach((server) => {
		if (!serverData[server.address]) {
			serverData[server.address] = {
				refreshing: true,
			}
		} else {
			serverData[server.address].refreshing = true
		}
	})

	if (!ping) {
		return
	}

	await Promise.all(
		[...new Set(servers.map((server) => server.address))].map((address) =>
			refreshServerData(serverData[address], protocolVersion, address),
		),
	)
}

export async function refreshWorld(worlds: World[], instanceId: string, worldPath: string) {
	const index = worlds.findIndex((w) => w.type === 'singleplayer' && w.path === worldPath)
	const newWorld = await get_singleplayer_world(instanceId, worldPath)
	if (index !== -1) {
		worlds[index] = newWorld
	} else {
		console.info(`Adding new world at path: ${worldPath}.`)
		worlds.push(newWorld)
	}
	sortWorlds(worlds)
}

export async function handleDefaultInstanceUpdateEvent(
	worlds: World[],
	instanceId: string,
	e: InstanceEvent,
) {
	if (e.event === 'world_updated') {
		await refreshWorld(worlds, instanceId, e.world)
	}

	if (e.event === 'server_joined') {
		const world = worlds.find(
			(w) =>
				w.type === 'server' &&
				(w.address === `${e.host}:${e.port}` || (e.port == 25565 && w.address == e.host)),
		)
		if (world) {
			world.last_played = e.timestamp
			sortWorlds(worlds)
		} else {
			console.error(`Could not find world for server join event: ${e.host}:${e.port}`)
		}
	}
}

export async function refreshWorlds(instanceId: string): Promise<World[]> {
	const worlds = await get_instance_worlds(instanceId).catch((err) => {
		console.error(`Error refreshing worlds for instance: ${instanceId}`, err)
	})
	if (worlds) {
		sortWorlds(worlds)
	}

	return worlds ?? []
}

export function hasServerQuickPlaySupport(gameVersions: GameVersion[], currentVersion: string) {
	if (!gameVersions.length) {
		return true
	}

	const versionIndex = gameVersions.findIndex((v) => v.version === currentVersion)
	const targetIndex = gameVersions.findIndex((v) => v.version === 'a1.0.5_01')

	return versionIndex === -1 || targetIndex === -1 || versionIndex <= targetIndex
}

export function hasWorldQuickPlaySupport(gameVersions: GameVersion[], currentVersion: string) {
	if (!gameVersions.length) {
		return false
	}

	const versionIndex = gameVersions.findIndex((v) => v.version === currentVersion)
	const targetIndex = gameVersions.findIndex((v) => v.version === '23w14a')

	return versionIndex !== -1 && targetIndex !== -1 && versionIndex <= targetIndex
}

export type InstanceEvent = InstancePayload
