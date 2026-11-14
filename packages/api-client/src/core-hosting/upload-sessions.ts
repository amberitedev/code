import { type InstanceUpload, instanceFiles } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import type { KyrosUploadSessionsV1Module } from '../modules/kyros/upload-sessions/v1'
import type { UploadHandle, UploadProgress } from '../types/upload'
import { type Adapter, coreOf } from './adapter'

/** A Core server has one world, with the server's id. */
export function uploadSessions(
	client: AbstractModrinthClient,
): Adapter<KyrosUploadSessionsV1Module> {
	const api = (worldId: string) => coreOf(client, worldId)
	return {
		create: async (scope, worldId) =>
			(await api(worldId)).call(instanceFiles.OpenInstanceUpload, {
				params: { id: worldId },
				body: { scope },
			}),
		get: async (scope, worldId) => ({
			session: await (
				await api(worldId)
			).call(instanceFiles.GetInstanceUpload, { params: { id: worldId }, query: { scope } }),
		}),
		uploadFiles(_scope, worldId, uploadId, files, options) {
			return uploadEach(client, worldId, uploadId, files, options?.onProgress)
		},
		finalize: async (_scope, worldId, uploadId) =>
			(await api(worldId)).call(instanceFiles.FinalizeInstanceUpload, {
				params: { id: worldId, upload: uploadId },
			}),
		cancel: async (_scope, worldId, uploadId) =>
			(await api(worldId)).call(instanceFiles.CancelInstanceUpload, {
				params: { id: worldId, upload: uploadId },
			}),
	}
}

/**
 * Each file as its own upload, one after another, with their progress as one. The engine's route
 * takes one file per request, as its body.
 */
function uploadEach(
	client: AbstractModrinthClient,
	serverId: string,
	uploadId: string,
	files: { file: File | Blob; filename: string }[],
	onProgress?: (progress: UploadProgress) => void,
): UploadHandle<InstanceUpload> {
	const listeners = onProgress ? [onProgress] : []
	const total = files.reduce((sum, { file }) => sum + file.size, 0)
	const report = (loaded: number) => {
		for (const listener of listeners)
			listener({ loaded, total, progress: total > 0 ? loaded / total : 1 })
	}
	let current: UploadHandle<InstanceUpload> | undefined
	let cancelled = false
	const promise = (async () => {
		const cores = client.archon.cores_v1
		const base = await cores.url((await cores.coreOf(serverId)).id)
		// `upload` puts a version segment between the base and the path: the route's first segment.
		const [, version, ...path] = instanceFiles.AddInstanceUploadFile.path
			.replace('{id}', encodeURIComponent(serverId))
			.replace('{upload}', encodeURIComponent(uploadId))
			.split('/')
		let done = 0
		let session: InstanceUpload | undefined
		for (const { file, filename } of files) {
			if (cancelled) throw new ModrinthApiError('Upload cancelled')
			current = client.upload<InstanceUpload>(`/${path.join('/')}`, {
				api: base,
				version,
				file,
				params: { path: filename },
				onProgress: ({ loaded }) => report(done + loaded),
			})
			session = await current.promise
			done += file.size
			report(done)
		}
		if (!session) throw new ModrinthApiError('No files to upload', { statusCode: 400 })
		return session
	})()
	const handle: UploadHandle<InstanceUpload> = {
		promise,
		onProgress(callback) {
			listeners.push(callback)
			return handle
		},
		cancel() {
			cancelled = true
			current?.cancel()
		},
	}
	return handle
}
