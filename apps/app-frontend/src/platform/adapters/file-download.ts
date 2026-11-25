import { ApiCallError, ModrinthApiError } from '@modrinth/api-client'
import type { FileDownloadProvider } from '@modrinth/ui'

import { save } from '@/platform/dialog'
import { cancelOp, type ExternalFileProgress, saveExternal } from '@/platform/ops'

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

/** Saves a Core's download (see `FileDownload`) to a file the user picks, with its progress. */
export function createFileDownload(trackDownload: TrackFileDownload): FileDownloadProvider {
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
			const originHeaders = { Origin: window.location.origin }
			const saveUrl = async (url: string) => {
				if (cancelled) throw new Error('Download cancelled')
				const { error, response } = await saveExternal(
					{ op: saveId, path, url, headers: originHeaders },
					onProgress,
				)
				if (error) throw new ApiCallError(0, error.error, error.description)
				if (response) {
					// Typed on the answer's error code when it is one of ours.
					throw ApiCallError.from(
						new ModrinthApiError(`Download failed with status ${response.status}`, {
							statusCode: response.status,
							responseData: parseBody(response.body),
						}),
					)
				}
			}

			try {
				await saveUrl(file.url)
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

/** A JSON error body as data, so a Core's `invalid_credentials` 401 can sign in again. */
function parseBody(body: string): unknown {
	try {
		return JSON.parse(body)
	} catch {
		return body
	}
}
