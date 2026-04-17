/**
 * Mocked worlds helper - returns empty/fake data
 * TODO: connect to backend API - replace with real API calls
 */

import type { GameVersion } from '@modrinth/ui'

export type WorldType = 'singleplayer' | 'server'
export type DisplayStatus = 'normal' | 'hidden' | 'favorite'
export type SingleplayerGameMode = 'survival' | 'creative' | 'adventure' | 'spectator'
export type ServerPackStatus = 'enabled' | 'disabled' | 'prompt'
export type ProtocolVersion = {
	version: number
	legacy: boolean
}

export type BaseWorld = {
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
	address: string
	pack_status: ServerPackStatus
	project_id?: string
	content_kind?: string
}

export type World = SingleplayerWorld | ServerWorld
export type WorldWithProfile = {
	profile: string
} & World

export type ServerStatus = {
	description?: string
	players?: {
		max: number
		online: number
		sample: { name: string; id: string }[]
	}
	version?: {
		name: string
		protocol: number
		legacy: boolean
	}
	favicon?: string
	enforces_secure_chat: boolean
	ping?: number
}

export type ServerData = {
	refreshing: boolean
	lastSuccessfulRefresh?: number
	status?: ServerStatus
	rawMotd?: string
	renderedMotd?: string
}

export type ProfileEvent = { profile_path_id: string } & (
	| {
			event: 'servers_updated'
	  }
	| {
			event: 'world_updated'
			world: string
	  }
	| {
			event: 'server_joined'
			host: string
			port: number
			timestamp: string
	  }
)

const DEFAULT_SERVER_PORT = 25565

export async function get_recent_worlds(_limit: number, _displayStatuses?: DisplayStatus[]): Promise<WorldWithProfile[]> {
	return []
}

export async function get_profile_worlds(_path: string): Promise<World[]> {
	return []
}

export async function get_singleplayer_world(_instance: string, _world: string): Promise<SingleplayerWorld> {
	return {
		name: '',
		type: 'singleplayer',
		path: '',
		game_mode: 'survival',
		hardcore: false,
		locked: false,
		display_status: 'normal',
	}
}

export async function set_world_display_status(_instance: string, _worldType: WorldType, _worldId: string, _displayStatus: DisplayStatus): Promise<void> {
	// no-op
}

export async function rename_world(_instance: string, _world: string, _newName: string): Promise<void> {
	// no-op
}

export async function reset_world_icon(_instance: string, _world: string): Promise<void> {
	// no-op
}

export async function backup_world(_instance: string, _world: string): Promise<number> {
	return 0
}

export async function delete_world(_instance: string, _world: string): Promise<void> {
	// no-op
}

export async function add_server_to_profile(_path: string, _name: string, _address: string, _packStatus: ServerPackStatus, _projectId?: string, _contentKind?: string): Promise<number> {
	return 0
}

export async function edit_server_in_profile(_path: string, _index: number, _name: string, _address: string, _packStatus: ServerPackStatus): Promise<void> {
	// no-op
}

export async function remove_server_from_profile(_path: string, _index: number): Promise<void> {
	// no-op
}

export async function get_profile_protocol_version(_path: string): Promise<ProtocolVersion | null> {
	return null
}

export async function get_server_status(_address: string, _protocolVersion: ProtocolVersion | null = null): Promise<ServerStatus> {
	return {
		players: { max: 0, online: 0, sample: [] },
		version: { name: '', protocol: 0, legacy: false },
		enforces_secure_chat: false,
	}
}

export async function start_join_singleplayer_world(_path: string, _world: string): Promise<unknown> {
	return null
}

export async function start_join_server(_path: string, _address: string): Promise<unknown> {
	return null
}

export async function showWorldInFolder(_instancePath: string, _worldPath: string) {
	// no-op
}

export function getWorldIdentifier(_world: World) {
	return ''
}

export function sortWorlds(_worlds: World[]) {
	// no-op
}

export function isSingleplayerWorld(_world: World): _world is SingleplayerWorld {
	return false
}

export function isServerWorld(_world: World): _world is ServerWorld {
	return false
}

function parseServerPort(_port: string): number | null {
	return null
}

function parseServerHost(_address: string): string {
	return ''
}

function isIPv4Host(_host: string): boolean {
	return false
}

export function normalizeServerAddress(_address: string): string {
	return ''
}

export function getServerDomainKey(_address: string): string {
	return ''
}

export function resolveManagedServerWorld(
	_worlds: World[],
	_managedName: string | null | undefined,
	_managedAddress: string | null | undefined,
): ServerWorld | null {
	return null
}

export async function getServerLatency(_address: string, _protocolVersion: ProtocolVersion | null = null): Promise<number | undefined> {
	return 0
}

export async function refreshServerData(_serverData: ServerData, _protocolVersion: ProtocolVersion | null, _address: string): Promise<void> {
	// no-op
}

export function refreshServers(_worlds: World[], _serverData: Record<string, ServerData>, _protocolVersion: ProtocolVersion | null) {
	// no-op
}

export async function refreshWorld(_worlds: World[], _instancePath: string, _worldPath: string) {
	// no-op
}

export async function handleDefaultProfileUpdateEvent(_worlds: World[], _instancePath: string, _e: ProfileEvent) {
	// no-op
}

export async function refreshWorlds(_instancePath: string): Promise<World[]> {
	return []
}

export function hasServerQuickPlaySupport(_gameVersions: GameVersion[], _currentVersion: string) {
	return true
}

export function hasWorldQuickPlaySupport(_gameVersions: GameVersion[], _currentVersion: string) {
	return false
}

export async function get_profile_worlds_for_server(_path: string): Promise<World[]> {
	return []
}

export async function get_profile_worlds_for_singleplayer(_path: string): Promise<World[]> {
	return []
}
