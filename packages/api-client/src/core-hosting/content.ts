import {
	type ContentItem,
	core,
	instanceContent,
	installs,
	instanceSharing,
	type LinkedModpackInfo,
	type ProjectType,
} from '@amberite/contracts'

import type { ContractClient } from '../contracts/client'
import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonContentV1Module } from '../modules/archon/content/v1'
import type { Archon } from '../modules/archon/types'
import type { UploadHandle, UploadProgress } from '../types/upload'
import { type Adapter, coreOf } from './adapter'

declare module '../modules/archon/content/v1' {
	interface ArchonContentV1Module {
		share(
			serverId: string,
			worldId: string,
			configPaths?: string[],
			options?: { timeout?: number; restart?: boolean; removedConfigPaths?: string[] },
		): Promise<Archon.Content.v1.ShareWorldContentResponse>
		/** Core only. Owner only. The versions of the server's own share, newest first. */
		getShareHistory(
			serverId: string,
			worldId: string,
		): Promise<Archon.Content.v1.SharedInstanceHistoryEntry[]>
		/** Core only. Owner only. Puts the server back to a version of its share, left to push. */
		restoreShareVersion(serverId: string, worldId: string, version: number): Promise<void>
		/** Core only. Owner only. The latest version stays. */
		deleteShareVersion(serverId: string, worldId: string, version: number): Promise<void>
		/** Core only. Installs a `.mrpack` over the server. */
		uploadModpackFile(
			serverId: string,
			worldId: string,
			pack: File | Blob,
			options?: {
				properties?: Archon.Content.v1.PropertiesFields
				onProgress?: (progress: UploadProgress) => void
			},
		): UploadHandle<void>
	}
}

type Kind = Archon.Content.v1.AddonKind

const KINDS = {
	mod: 'mod',
	plugin: 'plugin',
	datapack: 'datapack',
	resourcepack: 'resourcepack',
	shaderpack: 'shader',
} as const satisfies Record<ProjectType, Kind>

export function unsupported(what: string): never {
	throw new ApiCallError(501, 'unsupported', `A Core does not ${what}.`)
}

function notFound(what: string): never {
	throw new ApiCallError(404, 'not_found', `The server has no ${what}.`)
}

/** Settings an install brings, written before it. */
async function writeProperties(
	api: ContractClient,
	serverId: string,
	given?: Archon.Content.v1.PropertiesFields | null,
) {
	if (!given) return
	const changes: Record<string, string | null> = {}
	for (const [field, value] of Object.entries(given.known))
		if (typeof value === 'string') changes[field.replace(/_/g, '-')] = value
	Object.assign(changes, given.custom)
	await api.call(core.ChangeProperties, { params: { id: serverId }, body: { properties: changes } })
}

/** An item of the engine's content list, from its pack or added. */
type Listed = { item: ContentItem; fromModpack: boolean }

function addon({ item, fromModpack }: Listed): Archon.Content.v1.Addon {
	const sides = item.server
	return {
		id: item.id,
		filename: item.file_name,
		filesize: item.size,
		disabled: !item.enabled,
		disabled_server: sides?.disabled_server ?? false,
		disabled_player: sides?.disabled_player ?? false,
		side_toggle_unlocked: sides?.sides_unlocked ?? false,
		manifest: null,
		kind: KINDS[item.project_type],
		from_modpack: fromModpack,
		status: 'installed',
		pack_client_retained: sides?.server_override ?? false,
		pack_client_depends: false,
		needs_side_type_review: false,
		has_update: item.update_version_id,
		name: item.project?.title ?? item.embedded_metadata?.name ?? null,
		project_id: item.project?.id ?? null,
		version: item.version && {
			id: item.version.id,
			name: item.version.version_number,
			environment: item.environment,
		},
		owner: item.owner && {
			id: item.owner.id,
			name: item.owner.name,
			type: item.owner.type,
			icon_url: item.owner.avatar_url,
		},
		icon_url: item.project?.icon_url ?? null,
		installed: sides?.installed ?? true,
	}
}

function modpack(info: LinkedModpackInfo): Archon.Content.v1.ModpackFields {
	return {
		spec: {
			platform: 'modrinth',
			project_id: info.project.id,
			version_id: info.version?.id ?? '',
		},
		has_update: info.update_version_id,
		title: info.project.title,
		description: info.project.description,
		icon_url: info.project.icon_url,
		owner: info.owner && {
			id: info.owner.id,
			name: info.owner.name,
			type: info.owner.type,
			icon_url: info.owner.avatar_url,
		},
		version_number: info.version?.version_number ?? null,
		date_published: info.version?.date_published ?? null,
		downloads: info.project.downloads,
		followers: info.project.followers,
	}
}

/** A server's content and share, as its instance on the Core's engine has them. */
export function content(client: AbstractModrinthClient): Adapter<ArchonContentV1Module> {
	const params = (serverId: string) => ({ params: { id: serverId } })
	const read = (serverId: string) => ({ ...params(serverId), query: {} })

	async function listed(serverId: string): Promise<Listed[]> {
		const api = await coreOf(client, serverId)
		const [own, pack] = await Promise.all([
			api.call(instanceContent.ListContentItems, read(serverId)),
			api.call(instanceContent.ListLinkedModpackContent, read(serverId)),
		])
		return [
			...own.map((item) => ({ item, fromModpack: false })),
			...pack.map((item) => ({ item, fromModpack: true })),
		]
	}

	/** The engine's path of the addon the panel names by kind and file name. */
	async function path(serverId: string, kind: Kind, filename: string) {
		const found = (await listed(serverId)).find(
			({ item }) => KINDS[item.project_type] === kind && item.file_name === filename,
		)
		return found?.item.file_path ?? notFound(`${kind} named ${filename}`)
	}

	async function toggle(
		serverId: string,
		request: Archon.Content.v1.RemoveAddonRequest,
		enabled: boolean,
	) {
		await (
			await coreOf(client, serverId)
		).call(instanceContent.ToggleProject, {
			...params(serverId),
			body: {
				project_path: await path(serverId, request.kind, request.filename),
				desired_enabled: enabled,
			},
		})
	}

	async function remove(serverId: string, request: Archon.Content.v1.RemoveAddonRequest) {
		await (
			await coreOf(client, serverId)
		).call(instanceContent.RemoveProject, {
			...params(serverId),
			body: { project_path: await path(serverId, request.kind, request.filename) },
		})
	}

	async function setSide(
		serverId: string,
		request: Archon.Content.v1.SetAddonEnabledRequest,
		side: 'server' | 'player',
	) {
		await (
			await coreOf(client, serverId)
		).call(instanceContent.SetSide, {
			...params(serverId),
			body: {
				project_path: await path(serverId, request.kind, request.filename),
				side,
				enabled: request.enabled,
			},
		})
	}

	async function add(serverId: string, request: Archon.Content.v1.AddAddonRequest) {
		await (
			await coreOf(client, serverId)
		).call(instanceContent.InstallProjectWithDependencies, {
			...params(serverId),
			body: {
				project_id: request.project_id,
				version_id: request.version_id ?? null,
				content_type: request.kind ?? 'mod',
			},
		})
	}

	async function update(serverId: string, request: Archon.Content.v1.UpdateAddonRequest) {
		const found = (await listed(serverId)).find(({ item }) => item.file_name === request.filename)
		if (!found) return notFound(`file ${request.filename}`)
		const projectPath = found.item.file_path
		const versionId = request.version_id ?? found.item.update_version_id
		const api = await coreOf(client, serverId)
		if (versionId)
			await api.call(instanceContent.SwitchProjectVersion, {
				...params(serverId),
				body: { project_path: projectPath, version_id: versionId },
			})
		else
			await api.call(instanceContent.UpdateProject, {
				...params(serverId),
				body: { project_path: projectPath },
			})
	}

	async function linked(serverId: string) {
		return (await coreOf(client, serverId)).call(instanceContent.GetLinkedModpack, read(serverId))
	}

	return {
		async getAddons(serverId, _worldId, options) {
			const api = await coreOf(client, serverId)
			const [server, pack, all] = await Promise.all([
				api.call(core.GetServer, params(serverId)),
				linked(serverId),
				options?.addons === false ? null : listed(serverId),
			])
			const addons =
				all
					?.map(addon)
					.filter(
						(item) =>
							(options?.from_modpack === undefined || item.from_modpack === options.from_modpack) &&
							(options?.disabled === undefined || item.disabled === options.disabled) &&
							(options?.updates !== true || item.has_update !== null) &&
							options?.needs_side_type_review !== true,
					) ?? null
			return {
				modloader: server.loader,
				modloader_version: server.loader_version,
				game_version: server.game_version,
				update_channel: server.update_channel,
				modpack: pack && modpack(pack),
				installing: server.installing ? 'modpack' : undefined,
				files_need_side_type_review: 0,
				addons,
			}
		},
		addAddon: (serverId, _worldId, request) => add(serverId, request),
		async addAddons(serverId, _worldId, addons) {
			for (const request of addons) await add(serverId, request)
		},
		deleteAddon: (serverId, _worldId, request) => remove(serverId, request),
		async deleteAddons(serverId, _worldId, requests) {
			for (const request of requests) await remove(serverId, request)
		},
		disableAddon: (serverId, _worldId, request) => toggle(serverId, request, false),
		enableAddon: (serverId, _worldId, request) => toggle(serverId, request, true),
		async disableAddons(serverId, _worldId, requests) {
			for (const request of requests) await toggle(serverId, request, false)
		},
		async enableAddons(serverId, _worldId, requests) {
			for (const request of requests) await toggle(serverId, request, true)
		},
		setAddonEnabledServer: (serverId, _worldId, request) => setSide(serverId, request, 'server'),
		setAddonEnabledPlayer: (serverId, _worldId, request) => setSide(serverId, request, 'player'),
		async setAddonSideToggleLocked(serverId, _worldId, request) {
			await (
				await coreOf(client, serverId)
			).call(instanceContent.SetSidesUnlocked, {
				...params(serverId),
				body: {
					project_path: await path(serverId, request.kind, request.filename),
					unlocked: !request.locked,
				},
			})
		},
		async completeSideTypeReview(serverId) {
			await (
				await coreOf(client, serverId)
			).call(instanceContent.CompleteSideReview, params(serverId))
		},
		async installContent(serverId, _worldId, request) {
			const api = await coreOf(client, serverId)
			switch (request.content_variant) {
				case 'bare':
					await writeProperties(api, serverId, request.properties)
					await api.call(core.InstallLoader, {
						...params(serverId),
						body: {
							loader: request.loader === 'neo_forge' ? 'neoforge' : request.loader,
							loader_version: request.version || null,
							game_version: request.game_version ?? '',
						},
					})
					return
				case 'modpack':
					if (request.spec.platform !== 'modrinth')
						return unsupported('install a pack file it was not sent; upload it instead')
					await writeProperties(api, serverId, request.properties)
					await api.call(instanceContent.InstallModpack, {
						...params(serverId),
						body: { project_id: request.spec.project_id, version_id: request.spec.version_id },
					})
					return
				case 'instance':
					return unsupported('follow an App instance yet')
			}
		},
		async repair(serverId) {
			await (await coreOf(client, serverId)).call(instanceContent.RepairContent, params(serverId))
		},
		restoreClientFiles: () => unsupported('keep client files to restore'),
		async unlinkModpack(serverId) {
			await (await coreOf(client, serverId)).call(instanceContent.UnlinkModpack, params(serverId))
		},
		async getAddonUpdate(serverId, _worldId, filename) {
			const found = (await listed(serverId)).find(({ item }) => item.file_name === filename)
			return found ? addon(found) : notFound(`file ${filename}`)
		},
		updateAddon: (serverId, _worldId, request) => update(serverId, request),
		async updateAddons(serverId, _worldId, addons) {
			for (const request of addons) await update(serverId, request)
		},
		async getModpackUpdate(serverId) {
			const pack = await linked(serverId)
			return pack ? modpack(pack) : notFound('modpack')
		},
		async updateModpack(serverId) {
			const pack = await linked(serverId)
			if (!pack?.update_version_id) return
			await (
				await coreOf(client, serverId)
			).call(instanceContent.UpdateModpackVersion, {
				...params(serverId),
				body: { version_id: pack.update_version_id },
			})
		},
		getUpdateGameVersionPreview: () => unsupported('preview a Minecraft version change'),
		applyGameVersionUpdate: () => unsupported('change the Minecraft version of a pack in place'),
		async share(
			serverId: string,
			_worldId: string,
			configPaths: string[] = [],
			options?: { restart?: boolean; removedConfigPaths?: string[] },
		) {
			return (await coreOf(client, serverId)).call(core.ShareServer, {
				...params(serverId),
				body: {
					file_paths: configPaths,
					removed_paths: options?.removedConfigPaths ?? [],
					restart: options?.restart ?? false,
				},
			})
		},
		async unshare(serverId) {
			await (
				await coreOf(client, serverId)
			).call(instanceSharing.UnpublishSharedInstance, params(serverId))
		},
		async getShareDiff(serverId) {
			const api = await coreOf(client, serverId)
			const [preview, server] = await Promise.all([
				api.call(instanceSharing.GetSharedInstancePublishPreview, params(serverId)),
				api.call(core.GetServer, params(serverId)),
			])
			if (!preview) return notFound('share')
			const hasChanges = preview.diffs.length > 0
			return {
				shared_instance_id: preview.sharedInstanceId,
				latest_version: preview.latestVersion,
				has_changes: hasChanges,
				requires_restart: hasChanges && server.power === 'running',
				diffs: preview.diffs,
			}
		},
		async getShareableFiles(serverId) {
			const preview = await (
				await coreOf(client, serverId)
			).call(instanceSharing.GetSharedInstancePublishPreview, params(serverId))
			return { files: preview?.files ?? [], truncated: false }
		},
		async getShareHistory(serverId) {
			return (await coreOf(client, serverId)).call(
				installs.GetSharedInstanceHistory,
				params(serverId),
			)
		},
		async restoreShareVersion(serverId, _worldId, version) {
			await (
				await coreOf(client, serverId)
			).call(installs.RestoreSharedInstanceVersion, { params: { id: serverId, version } })
		},
		async deleteShareVersion(serverId, _worldId, version) {
			await (
				await coreOf(client, serverId)
			).call(installs.DeleteSharedInstanceVersion, { params: { id: serverId, version } })
		},
		uploadModpackFile: (serverId, _worldId, pack, options) =>
			installPackFile(client, serverId, pack, undefined, options?.properties, options?.onProgress),
	}
}

/**
 * Install `pack` over the server once `given` settings are written, then answer `answer`. The
 * Core reports no upload progress, so the handle reports it done once the Core has the pack.
 */
export function installPackFile<T>(
	client: AbstractModrinthClient,
	serverId: string,
	pack: Blob,
	answer: T,
	given?: Archon.Content.v1.PropertiesFields | null,
	onProgress?: (progress: UploadProgress) => void,
): UploadHandle<T> {
	const listeners = onProgress ? [onProgress] : []
	const promise = (async () => {
		const api = await coreOf(client, serverId)
		await writeProperties(api, serverId, given)
		await api.call(core.InstallModpackFile, { params: { id: serverId }, body: pack })
		for (const listener of listeners) listener({ loaded: pack.size, total: pack.size, progress: 1 })
		return answer
	})()
	const handle: UploadHandle<T> = {
		promise,
		onProgress(callback) {
			listeners.push(callback)
			return handle
		},
		cancel() {},
	}
	return handle
}
