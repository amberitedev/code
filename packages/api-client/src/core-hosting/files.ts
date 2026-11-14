import { instanceFiles } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import type { KyrosFilesV0Module } from '../modules/kyros/files/v0'
import type { KyrosFilesV1Module } from '../modules/kyros/files/v1'
import type { Kyros } from '../modules/kyros/types'
import { nodeAuthState } from '../state/node-auth'
import { type Adapter, coreOf } from './adapter'
import { link, linkUrl, node, serverPath } from './links'

/** The server whose panel is open: upstream's v0 file methods name none, and go to its node. */
async function openServer(client: AbstractModrinthClient) {
	if (!nodeAuthState.getAuth?.()) await nodeAuthState.refreshAuth?.()
	const auth = nodeAuthState.getAuth?.()
	if (!auth) throw new ModrinthApiError('No server is open', { statusCode: 400 })
	const id = node(auth.url).serverId
	return { params: { id }, api: await coreOf(client, id) }
}

export function filesV0(client: AbstractModrinthClient): Adapter<KyrosFilesV0Module> {
	return {
		async listDirectory(path, page = 1, pageSize = 100) {
			const { params, api } = await openServer(client)
			const items = (await api.call(instanceFiles.ListInstanceFiles, { params, query: { path } }))
				.map((item): Kyros.Files.v0.DirectoryItem => ({
					name: item.name,
					type: item.type,
					path: `/${item.path}`,
					modified: item.modified,
					created: item.created,
					size: item.size ?? undefined,
					count: item.count ?? undefined,
				}))
				.sort(
					(a, b) =>
						Number(b.type === 'directory') - Number(a.type === 'directory') ||
						a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }),
				)
			const size = Math.min(Math.max(pageSize, 1), 1000)
			return {
				items: items.slice((page - 1) * size, page * size),
				total: items.length,
				current: page,
			}
		},
		async createFileOrFolder(path, type) {
			const { params, api } = await openServer(client)
			if (type === 'directory') {
				await api.call(instanceFiles.CreateInstanceDirectory, { params, query: { path } })
			} else {
				await api.call(instanceFiles.WriteInstanceFile, {
					params,
					query: { path, create_only: true },
					body: new Blob([]),
				})
			}
		},
		async downloadFile(path) {
			const { params, api } = await openServer(client)
			return api.call(instanceFiles.ReadInstanceFile, { params, query: { path } })
		},
		async updateFile(path, content) {
			const { params, api } = await openServer(client)
			await api.call(instanceFiles.WriteInstanceFile, {
				params,
				query: { path, create_only: false },
				body: typeof content === 'string' ? new Blob([content]) : content,
			})
		},
		async moveFileOrFolder(source, destination) {
			const { params, api } = await openServer(client)
			await api.call(instanceFiles.MoveInstanceFile, { params, body: { source, destination } })
		},
		async deleteFileOrFolder(path, recursive) {
			const { params, api } = await openServer(client)
			await api.call(instanceFiles.DeleteInstanceFile, { params, query: { path, recursive } })
		},
		async extractFile(path, override = true, dry = false, target = '/') {
			const { params, api } = await openServer(client)
			const result = await api.call(instanceFiles.ExtractInstanceZip, {
				params,
				body: { path, target, override_conflicts: override, dry_run: dry },
			})
			return {
				modpack_name: result?.modpack_name ?? null,
				conflicting_files: (result?.conflicting_files ?? []).map((file) => `/${file}`),
			}
		},
		// Unpacking ends within its request, so no operation is ever left to dismiss or cancel.
		async modifyOperation() {},
	}
}

export function filesV1(client: AbstractModrinthClient): Adapter<KyrosFilesV1Module> {
	// `getFileDownloadUrl` is not given the file, so each token keeps the path it was made for.
	const files = new Map<string, string>()
	const worldZip = (nodeUrlHost: string, worldId: string) =>
		serverPath(nodeUrlHost, `/v1/worlds/${encodeURIComponent(worldId)}/files/download-full-zip`)
	return {
		async authorizeFileDownload(nodeUrlHost, _worldId, path) {
			const { serverId } = node(nodeUrlHost)
			const file = `/instances/${serverId}/files/content?path=${encodeURIComponent(path)}`
			const made = await link(client, nodeUrlHost, file)
			files.set(made.token, file)
			return made
		},
		getFileDownloadUrl(nodeUrlHost, _worldId, token) {
			const file = files.get(token)
			if (!file) throw new ModrinthApiError('This download link was used', { statusCode: 401 })
			files.delete(token)
			return linkUrl(nodeUrlHost, file, token)
		},
		authorizeFullWorldDownload: (nodeUrlHost, worldId) =>
			link(client, nodeUrlHost, worldZip(nodeUrlHost, worldId)),
		getFullWorldDownloadUrl: (nodeUrlHost, worldId, token) =>
			linkUrl(nodeUrlHost, worldZip(nodeUrlHost, worldId), token),
		async stat(worldId, { path }) {
			const stat = await (
				await coreOf(client, worldId)
			).call(instanceFiles.StatInstanceFile, { params: { id: worldId }, query: { path } })
			return {
				name: stat.name,
				full_path: `/${stat.path}`,
				size_bytes: stat.size,
				type: stat.type,
				mtime: stat.modified,
				ctime: stat.created,
			}
		},
		async createZip(worldId, data, onProgress) {
			await (
				await coreOf(client, worldId)
			).call(instanceFiles.ZipInstanceFiles, { params: { id: worldId }, body: data })
			onProgress?.({ progress: 1, done: true })
		},
	}
}
