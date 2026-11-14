import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonPropertiesV1Module } from '../modules/archon/properties/v1'
import type { Archon } from '../modules/archon/types'
import { type Adapter, coreOf } from './adapter'

type Known = Archon.Content.v1.KnownPropertiesFields

/** The known fields, as `server.properties` names them with dashes. */
const KNOWN = [
	'allow_cheats',
	'allow_flight',
	'difficulty',
	'enforce_whitelist',
	'force_gamemode',
	'gamemode',
	'generate_structures',
	'generator_settings',
	'hardcore',
	'level_seed',
	'level_type',
	'max_players',
	'max_tick_time',
	'motd',
	'pause_when_empty_seconds',
	'player_idle_timeout',
	'require_resource_pack',
	'resource_pack',
	'resource_pack_id',
	'resource_pack_sha1',
	'simulation_distance',
	'spawn_protection',
	'sync_chunk_writes',
	'view_distance',
	'white_list',
] as const satisfies readonly (keyof Known)[]

function isKnown(field: string): field is (typeof KNOWN)[number] {
	return KNOWN.some((known) => known === field)
}

/** Every key of the file, split into the known fields and the rest. */
export function fields(all: Record<string, string>): Archon.Content.v1.PropertiesFields {
	const known: Known = {}
	const custom: Record<string, string> = {}
	for (const [key, value] of Object.entries(all)) {
		const field = key.replace(/-/g, '_')
		if (isKnown(field)) known[field] = value
		else custom[key] = value
	}
	return { known, custom }
}

/** Editors and the owner only: the file can hold secrets, such as a resource pack URL. */
export function properties(client: AbstractModrinthClient): Adapter<ArchonPropertiesV1Module> {
	return {
		async getProperties(serverId) {
			const { properties } = await (
				await coreOf(client, serverId)
			).call(core.GetProperties, {
				params: { id: serverId },
			})
			return fields(properties)
		},
		async patchProperties(serverId, _worldId, body) {
			const changes: Record<string, string | null> = {}
			for (const [field, value] of Object.entries(body.known ?? {}))
				if (typeof value === 'string') changes[field.replace(/_/g, '-')] = value
			Object.assign(changes, body.custom)
			const { properties } = await (
				await coreOf(client, serverId)
			).call(core.ChangeProperties, {
				params: { id: serverId },
				body: { properties: changes },
			})
			return fields(properties)
		},
	}
}
