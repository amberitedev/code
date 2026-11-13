import {
	type AbstractModrinthClient,
	type DownloadSink,
	getNodeBaseUrl,
	ModrinthApiError,
} from '@modrinth/api-client'
import type { FileDownloadProvider } from '@modrinth/ui'

import { type ExternalFileProgress, saveExternal } from '@/platform/app-lib/files/commands'
import { save } from '@/platform/dialog'
import { cancelOp } from '@/platform/ops'

export interface FileDownloadTask {
	update: (progress: ExternalFileProgress) => void
	succeed: (filename: string) => void
	fail: (error: string) => void
	canceled: () => void
}

/** Registers a running download so the app can display its progress and cancel it. */
export type TrackFileDownload = (
	saveId: string,
	filename: string,
	onCancel: () => Promise<void>,
	server: { serverId?: string; serverName?: string },
) => FileDownloadTask

export function createFileDownload(
	client: AbstractModrinthClient,
	trackDownload: TrackFileDownload,
): FileDownloadProvider {
	return {
		async download(file) {
			const path = await save({
				defaultPath: file.filename.split(/[/\\]/).pop() || 'download',
				filters:
					file.type === 'server-mrpack'
						? [{ name: 'Modrinth Modpack', extensions: ['mrpack'] }]
						: file.type === 'server-backup' || file.type === 'server-world'
							? [{ name: 'ZIP archive', extensions: ['zip'] }]
							: [],
			})
			if (!path) return false

			const saveId = crypto.randomUUID()
			let cancelled = false
			let finished = false
			const savedFilename = path.split(/[/\\]/).pop() ?? path
			const task = trackDownload(
				saveId,
				file.filename,
				async () => {
					cancelled = true
					await cancelOp(saveId).catch(() => undefined)
				},
				{ serverId: file.serverId, serverName: file.serverName },
			)
			const onProgress = (progress: ExternalFileProgress) => {
				if (cancelled || finished) return
				task.update(progress)
			}

			// A Core download goes through the App's local proxy, which serves only the App:
			// the native download names the App's origin, as the WebView's own requests do.
			const originHeaders: Record<string, string> = client.selfHostedHosting
				? { Origin: window.location.origin }
				: {}
			const saveUrl = async (url: string, headers: Record<string, string> = {}) => {
				if (cancelled) throw new Error('Download cancelled')
				const { error, response } = await saveExternal(
					{ op: saveId, path, url, headers: { ...originHeaders, ...headers } },
					onProgress,
				)
				if (error) throw new ModrinthApiError(error.description)
				if (response) {
					throw new ModrinthApiError(`Download failed with status ${response.status}`, {
						statusCode: response.status,
					})
				}
			}
			const sink: DownloadSink = (url, options) => saveUrl(url, options.headers)

			try {
				switch (file.type) {
					case 'server-file': {
						const { token } = await client.kyros.files_v1.authorizeFileDownload(
							file.nodeUrlHost,
							file.worldId,
							file.path,
						)
						await saveUrl(
							client.kyros.files_v1.getFileDownloadUrl(file.nodeUrlHost, file.worldId, token),
						)
						break
					}
					case 'server-mrpack': {
						const { token } = await client.kyros.content_v1.authorizeMrpackExport(
							file.nodeUrlHost,
							file.worldId,
						)
						await saveUrl(
							client.kyros.content_v1.getMrpackExportUrl(file.nodeUrlHost, file.worldId, token),
						)
						break
					}
					case 'server-world': {
						const { token } = await client.kyros.files_v1.authorizeFullWorldDownload(
							file.nodeUrlHost,
							file.worldId,
						)
						await client.download(
							`/worlds/${encodeURIComponent(file.worldId)}/files/download-full-zip`,
							{
								api: getNodeBaseUrl(file.nodeUrlHost),
								version: 'v1',
								params: { token },
								skipAuth: true,
							},
							sink,
						)
						break
					}
					case 'server-backup': {
						const { token } = await client.kyros.backups_v1.authorizeDownload(
							file.nodeUrlHost,
							file.worldId,
							file.backupId,
						)
						await saveUrl(
							client.kyros.backups_v1.getDownloadUrl(
								file.nodeUrlHost,
								file.worldId,
								file.backupId,
								token,
							),
						)
						break
					}
				}
				task.succeed(savedFilename)
				return true
			} catch (error) {
				if (cancelled) {
					task.canceled()
					return false
				}
				task.fail(error instanceof Error ? error.message : String(error))
				throw error
			} finally {
				finished = true
			}
		},
	}
}
