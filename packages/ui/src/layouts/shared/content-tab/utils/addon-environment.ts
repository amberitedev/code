import type { ContentItem as EngineContentItem } from '@amberite/contracts/engine'
import type { Labrinth } from '@modrinth/api-client'

import type { ContentSide } from '../types'

/** A server's content file, as the engine lists it. */
type Addon = EngineContentItem

export function friendlyAddonName(addon: {
	file_name: string
	project?: { title: string } | null
	embedded_metadata?: { name?: string | null } | null
}): string {
	if (addon.project?.title) return addon.project.title
	if (addon.embedded_metadata?.name) return addon.embedded_metadata.name
	const lastDotIndex = addon.file_name.lastIndexOf('.')
	return lastDotIndex === -1 ? addon.file_name : addon.file_name.substring(0, lastDotIndex)
}

export function getAddonEnvironment(addon: Addon): Labrinth.Projects.v3.Environment | undefined {
	return addon.environment ?? undefined
}

export function hasDetectedEnvironment(addon: Addon) {
	const environment = getAddonEnvironment(addon)
	return environment !== undefined && environment !== 'unknown'
}

export function isPlayerOnlyContent(addon: Addon) {
	return addon.project_type === 'resourcepack' || addon.project_type === 'shaderpack'
}

export function isDisabledFor(addon: Addon, side: ContentSide) {
	return side === 'server' ? !!addon.server?.disabled_server : !!addon.server?.disabled_player
}

export function isEnabledForAnySide(addon: Addon) {
	return (
		(!isPlayerOnlyContent(addon) && !isDisabledFor(addon, 'server')) ||
		!isDisabledFor(addon, 'player')
	)
}

export function isIncompatibleEnvironment(addon: Addon, side: ContentSide) {
	const environment = getAddonEnvironment(addon)
	return side === 'server'
		? environment === 'client_only' || environment === 'singleplayer_only'
		: environment === 'server_only' || environment === 'dedicated_server_only'
}

/** Falls back to the project's side requirements when the addon has no detected environment. */
export function isRequiredEnvironment(
	addon: Addon,
	side: ContentSide,
	project?: Pick<Labrinth.Projects.v2.Project, 'server_side' | 'client_side'>,
) {
	const environment = getAddonEnvironment(addon)
	if (!environment || environment === 'unknown') {
		return side === 'server'
			? project?.server_side === 'required'
			: project?.client_side === 'required'
	}
	return side === 'server'
		? environment === 'client_and_server' ||
				environment === 'server_only' ||
				environment === 'server_only_client_optional' ||
				environment === 'dedicated_server_only'
		: environment === 'client_and_server' ||
				environment === 'client_only' ||
				environment === 'client_only_server_optional' ||
				environment === 'singleplayer_only'
}
