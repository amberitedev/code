import type { ContentItem as EngineContentItem } from '@amberite/contracts/engine'

import { defineMessages, type MessageDescriptor } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

import type { ContentEnabledForState, ContentItem, ContentSide, ContentWarningType } from '../types'
import {
	getAddonEnvironment,
	hasDetectedEnvironment,
	isDisabledFor,
	isIncompatibleEnvironment,
	isPlayerOnlyContent,
} from './addon-environment'

const messages = defineMessages({
	notInstalledLock: {
		id: 'hosting.content.enabled-for.client-only-not-installed-lock',
		defaultMessage: 'Players get this client-only file with the pack.',
	},
	resourcePackLock: {
		id: 'hosting.content.enabled-for.resource-pack-lock',
		defaultMessage: 'Resource packs can only be enabled for players.',
	},
	shaderLock: {
		id: 'hosting.content.enabled-for.shader-lock',
		defaultMessage: 'Shaders can only be enabled for players.',
	},
	notInstalledClientOnly: {
		id: 'hosting.content.instance.not-installed-client-only',
		defaultMessage: 'Not installed (client only)',
	},
	unknownEnvironment: {
		id: 'hosting.content.enabled-for.compatibility-unknown-warning',
		defaultMessage:
			"We couldn't determine whether this content works on the server, for players, or both.",
	},
	clientOnlyEnabledForServer: {
		id: 'hosting.content.enabled-for.client-only-on-server-warning',
		defaultMessage:
			'This client-only content is enabled on the server and may prevent the server from starting.',
	},
	singleplayerOnlyEnabledForServer: {
		id: 'hosting.content.enabled-for.singleplayer-only-on-server-warning',
		defaultMessage:
			'This singleplayer-only content is enabled on the server and may prevent the server from starting.',
	},
	serverOnlyEnabledForPlayers: {
		id: 'hosting.content.enabled-for.server-only-for-players-warning',
		defaultMessage:
			'This server-only content is included for players and may cause issues when they launch the game.',
	},
})

export interface ContentItemMapping {
	/** Defaults to each message's English text. */
	formatMessage?: (descriptor: MessageDescriptor) => string
	/** The URL of an embedded icon at a path on the host, when the page can load one from there. */
	iconUrl?: (path: string) => string
}

function englishMessage(descriptor: MessageDescriptor) {
	return descriptor.defaultMessage ?? descriptor.id
}

/**
 * The engine's content file as the content tab shows it, from an App or a Core. A file the engine
 * could not match to a Modrinth project has no `project` and is `external`. A server engine also
 * says where each file runs (`server`), which becomes the "Enabled for" column.
 */
export function toContentItem(
	item: EngineContentItem,
	mapping: ContentItemMapping = {},
	fromPack = false,
): ContentItem {
	const { project, version, owner, embedded_metadata: embedded } = item
	const iconPath = embedded?.icon_path
	return {
		id: item.file_path,
		file_name: item.file_name,
		file_path: item.file_path,
		size: item.size,
		enabled: item.enabled,
		locked: item.locked,
		project_type: item.project_type,
		project: project
			? {
					id: project.id,
					slug: project.slug ?? project.id,
					title: project.title,
					icon_url: project.icon_url ?? undefined,
					license: { ...project.license, url: project.license.url ?? undefined },
					categories: project.categories,
					additional_categories: project.additional_categories,
				}
			: undefined,
		external: !project,
		version: version
			? { ...version, date_published: version.date_published ?? undefined }
			: undefined,
		owner: owner ? { ...owner, avatar_url: owner.avatar_url ?? undefined } : undefined,
		has_update: item.has_update,
		update_version_id: item.update_version_id,
		date_added: item.date_added ?? undefined,
		environment: item.environment ?? undefined,
		source_kind: item.source_kind,
		synced_pack: item.synced_pack,
		embedded_metadata: embedded && {
			...embedded,
			icon_url: iconPath && mapping.iconUrl ? mapping.iconUrl(iconPath) : undefined,
		},
		...(item.server && serverSides(item, mapping.formatMessage ?? englishMessage, fromPack)),
	}
}

/** Where a server's file runs: the server, its players, or both, and what is wrong with that. */
function serverSides(
	item: EngineContentItem,
	formatMessage: (descriptor: MessageDescriptor) => string,
	fromPack: boolean,
): Pick<ContentItem, 'enabled' | 'enabledFor' | 'version' | 'pack_client_retained'> {
	const playerOnly = isPlayerOnlyContent(item)
	const serverEnabled = !playerOnly && !isDisabledFor(item, 'server')
	const playerEnabled = !isDisabledFor(item, 'player')
	// A client-only pack file the Core never placed: turning its server side on places it, and
	// players always get it with the pack.
	const notInstalled = item.server?.installed === false
	const lockedSides: ContentSide[] = notInstalled ? ['player'] : playerOnly ? ['server'] : []
	const warning = enabledForWarning(item, formatMessage, fromPack)
	const enabledFor: ContentEnabledForState = {
		server: serverEnabled,
		player: playerEnabled,
		locked: lockedSides.length > 0,
		lockedTooltip: notInstalled
			? formatMessage(messages.notInstalledLock)
			: playerOnly
				? formatMessage(
						item.project_type === 'resourcepack' ? messages.resourcePackLock : messages.shaderLock,
					)
				: undefined,
		disabledSides: lockedSides,
		warningTooltip: warning?.tooltip,
		warningKind: warning?.kind,
	}
	return {
		enabled: item.enabled && (serverEnabled || playerEnabled),
		enabledFor,
		pack_client_retained: item.server?.server_override,
		...(notInstalled && {
			version: {
				id: item.version?.id ?? item.file_name,
				version_number: formatMessage(messages.notInstalledClientOnly),
				file_name: item.file_name,
			},
		}),
	}
}

function enabledForWarning(
	item: EngineContentItem,
	formatMessage: (descriptor: MessageDescriptor) => string,
	fromPack: boolean,
): { kind: ContentWarningType; tooltip: string } | null {
	if (isPlayerOnlyContent(item)) return null
	if (!isDisabledFor(item, 'server')) {
		if (item.server?.server_override)
			return { kind: 'retained', tooltip: formatMessage(commonMessages.clientRetainedWarning) }
		if (isIncompatibleEnvironment(item, 'server')) {
			return {
				kind: 'environment',
				tooltip: formatMessage(
					getAddonEnvironment(item) === 'singleplayer_only'
						? messages.singleplayerOnlyEnabledForServer
						: messages.clientOnlyEnabledForServer,
				),
			}
		}
	}
	if (!isDisabledFor(item, 'player') && isIncompatibleEnvironment(item, 'player')) {
		return { kind: 'server-only', tooltip: formatMessage(messages.serverOnlyEnabledForPlayers) }
	}
	// A pack's files have the sides the pack gave them.
	if (!fromPack && !hasDetectedEnvironment(item))
		return { kind: 'unknown-environment', tooltip: formatMessage(messages.unknownEnvironment) }
	return null
}
